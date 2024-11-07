use crate::importer::extend_mail_parser::{get_reply_type_from_headers, MakeString};
use crate::importer::plain_text_to_html_converter;
use crate::tuta_imap::client::types::ImapMail;
use mail_parser::{Address, GetHeader, HeaderName, HeaderValue, MessageParser, PartType};
use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hash;
use std::time::SystemTime;
use tutasdk::date::DateTime;
use tutasdk::entities::generated::tutanota::{
    EncryptedMailAddress, ImportMailData, ImportMailDataMailReference, MailAddress, Recipients,
};
use tutasdk::CustomId;

// todo: this is used for DataTransferType, so id really dont have to be unique,
// but have to be valid length
const FIXED_CUSTOM_ID: &str = "____";

#[derive(Default)]
#[cfg_attr(test, derive(PartialEq, Debug))]
enum MailState {
    #[default]
    Received = 2,
    Sent = 1,
    Draft = 0,
}

#[repr(i64)]
#[derive(Default)]
#[cfg_attr(test, derive(PartialEq, Debug))]
enum ICalType {
    #[default]
    Nothing = 0,
    ICalPublish = 1,
    ICalRequest = 2,
    ICalAdd = 3,
    ICalCancel = 4,
    ICalRefresh = 5,
    ICalCounter = 6,
    ICalDeclineCounter = 7,
}

#[derive(Default)]
#[cfg_attr(test, derive(PartialEq, Debug))]
pub(super) enum ReplyType {
    #[default]
    Nothing = 0,
    Reply = 1,
    Forward = 2,
    ReplyForward = 3,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
enum ImportableMailAttachment {
    Attachment {
        filename: Option<String>,
        content_type: String,
        content_id: String,
        content: Vec<u8>,
        is_inline: bool,
    },
    AttachedMessage {
        message: ImportableMail,
    },
}

#[cfg_attr(test, derive(PartialEq, Debug))]
enum BodyText {
    Html(String),
    Plain(String),
}

#[derive(Default, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub struct MailContact {
    pub mail_address: String,
    pub name: String,
}

impl<'a> From<mail_parser::Addr<'a>> for MailContact {
    fn from(value: mail_parser::Addr) -> Self {
        Self {
            name: value.name.unwrap_or_default().to_string(),
            mail_address: value.address.unwrap_or_default().to_string(),
        }
    }
}

impl From<MailContact> for MailAddress {
    fn from(value: MailContact) -> Self {
        Self {
            _id: None,
            address: value.mail_address,
            name: value.name,
            contact: None,
            _finalIvs: Default::default(),
        }
    }
}

/// Input data for mail import service
#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct ImportableMail {
    pub headers_string: String,
    pub subject: String,
    pub html_body_text: String,
    pub attachments: Vec<ImportableMailAttachment>,

    pub date: Option<DateTime>,

    pub different_envelope_sender: Option<String>,
    pub from_addresses: Vec<MailContact>,
    pub to_addresses: Vec<MailContact>,
    pub cc_addresses: Vec<MailContact>,
    pub bcc_addresses: Vec<MailContact>,
    pub reply_to_addresses: Vec<MailContact>,

    pub ical_type: ICalType,
    pub reply_type: ReplyType,

    pub mail_state: MailState,
    pub is_phishing: bool, // https://turbo.fish/::%3Cphising%3E
    pub unread: bool,

    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

impl ImportableMail {
    /// Utility function to convert mail_parser::Address
    /// to a list of tutasdk::MailAddress
    /// in such a way that every address must have mail-address and optional name
    ///
    /// returns None, if any of the address have empty mail-address
    ///
    /// set the _id: of all mail address to random 4-byte long customId,
    /// this will only be valid in dataTransferType context
    fn map_to_tuta_mail_address(mail_parser_addresses: Cow<Address>) -> Vec<MailContact> {
        let address_list = match mail_parser_addresses.as_ref() {
            Address::List(address_list) => Cow::Borrowed(address_list),
            Address::Group(group_senders) => {
                let group_addresses = group_senders
                    .iter()
                    .map(|group| group.addresses.as_slice())
                    .collect::<Vec<_>>()
                    .concat();

                Cow::Owned(group_addresses)
            }
        };

        address_list
            .as_ref()
            .into_iter()
            .map(|address| MailContact {
                mail_address: address.address().unwrap_or_default().to_string(),
                name: address.name().unwrap_or_default().to_string(),
            })
            .collect()
    }

    // from the parsed message
    // return :
    // .0 a single string that ca be display as email in html format
    // .1 list of attachment found
    fn process_all_parts(
        parsed_message: &mail_parser::Message,
    ) -> Result<(String, Vec<ImportableMailAttachment>), MailParseError> {
        let mut email_body_as_html = String::new();
        let mut attachments = Vec::with_capacity(parsed_message.attachments.len());

        for part in &parsed_message.parts {
            match &part.body {
                PartType::Text(text) => {
                    let plain_text_as_html =
                        plain_text_to_html_converter::plain_text_to_html(text.to_string());
                    email_body_as_html.push_str(plain_text_as_html.as_ref())
                }
                PartType::Html(html_text) => {
                    email_body_as_html.push_str(html_text);
                }
                PartType::Message(attached_message) => {
                    let importable_mail = ImportableMail::try_from(attached_message.to_owned())?;
                    let this_attachment = ImportableMailAttachment::AttachedMessage {
                        message: importable_mail,
                    };
                    attachments.push(this_attachment);
                }

                PartType::Binary(binary_content) | PartType::InlineBinary(binary_content) => {
                    let is_inline = if matches!(part.body, PartType::InlineBinary(_)) {
                        true
                    } else if matches!(part.body, PartType::Binary(_)) {
                        false
                    } else {
                        unreachable!();
                    };

                    let content_type = part.headers.header_value(&HeaderName::ContentType).map(
                        |content_type_header| {
                            content_type_header
                                .as_content_type()
                                .expect("Content-Type header should be of type content type")
                        },
                    );

                    let filename = content_type
                        .map(mail_parser::ContentType::attributes)
                        .unwrap_or_default()
                        .unwrap_or_default()
                        .iter()
                        .filter(|(attribute_name, _)| attribute_name == "filename")
                        .map(|(_, file_name)| file_name.to_string())
                        // first attribute called 'filename'
                        .next();

                    let content_type = content_type
                        .map(MakeString::make_string)
                        .unwrap_or_default()
                        .to_string();

                    let content_id = part
                        .headers
                        .header_value(&HeaderName::ContentId)
                        .map(|content_type_header| {
                            content_type_header
                                .as_text()
                                .expect("Content-Id header should be of type text")
                        })
                        .unwrap_or("binary")
                        .to_string();
                    let content = binary_content.to_vec();
                    let this_attachment = ImportableMailAttachment::Attachment {
                        filename,
                        content_type,
                        content_id,
                        is_inline,
                        content,
                    };
                    attachments.push(this_attachment);
                }

                PartType::Multipart(multi_part_msg) => {
                    // no need to handle as we handle all other types separately (this is just a wrapper)
                    continue
                }
            }
        }

        Ok((email_body_as_html, attachments))
    }
}

impl From<ImportableMail> for ImportMailData {
    fn from(importable_mail: ImportableMail) -> Self {
        let ImportableMail {
            headers_string: headers,
            subject,
            html_body_text,
            different_envelope_sender,
            from_addresses,
            cc_addresses,
            bcc_addresses,
            to_addresses,
            date,
            reply_to_addresses,
            ical_type,
            reply_type,
            mail_state,
            is_phishing,
            unread,
            message_id,
            in_reply_to,
            references,
            attachments,
        } = importable_mail;

        let date = date.unwrap_or_else(|| DateTime::from_system_time(SystemTime::now()));

        let reply_tos = reply_to_addresses
            .into_iter()
            .map(|reply_to| EncryptedMailAddress {
                _id: Some(CustomId::from_custom_string(FIXED_CUSTOM_ID)),
                _finalIvs: Default::default(),
                name: reply_to.name,
                address: reply_to.mail_address,
            })
            .collect();

        let bcc_addresses = bcc_addresses.into_iter().map(Into::into).collect();
        let cc_addresses = cc_addresses.into_iter().map(Into::into).collect();
        let to_addresses = to_addresses.into_iter().map(Into::into).collect();
        let from_addresses: Vec<MailAddress> = from_addresses.into_iter().map(Into::into).collect();

        let references = references
            .into_iter()
            .map(|reference| ImportMailDataMailReference {
                _id: Some(CustomId::from_custom_string(FIXED_CUSTOM_ID)),
                reference,
            })
            .collect();

        ImportMailData {
            _id: Some(CustomId::from_custom_string(FIXED_CUSTOM_ID)),
            _finalIvs: HashMap::new(),
            compressedHeaders: headers,
            subject,
            compressedBodyText: html_body_text,
            differentEnvelopeSender: different_envelope_sender,
            sender: from_addresses
                .first()
                .cloned()
                .unwrap_or(MailContact::default().into()),
            recipients: Recipients {
                _id: Some(CustomId::from_custom_string(FIXED_CUSTOM_ID)),
                bccRecipients: bcc_addresses,
                ccRecipients: cc_addresses,
                toRecipients: to_addresses,
            },
            replyTos: reply_tos,
            unread,
            confidential: false,
            method: ical_type as i64,
            phishingStatus: if is_phishing { 1 } else { 0 },
            replyType: reply_type as i64,
            date,
            state: mail_state as i64,
            messageId: message_id,
            inReplyTo: in_reply_to,
            references,
            importedAttachments: vec![],
        }
    }
}

impl TryFrom<ImapMail> for ImportableMail {
    type Error = MailParseError;
    fn try_from(imap_mail: ImapMail) -> Result<Self, Self::Error> {
        let ImapMail { rfc822_full } = imap_mail;

        // parse the full mime message
        let imap_mail = MessageParser::new()
            .parse(rfc822_full.as_slice())
            .ok_or(MailParseError::InvalidMimeMessage)?;

        let mut importable_mail = Self::try_from(imap_mail)?;

        // example:
        // add more details from imap if given,
        importable_mail.is_phishing = false;
        importable_mail.unread = true;

        Ok(importable_mail)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MailParseError {
    InconsistentParts(&'static str),
    NoSentDate,
    NoRecipient,
    NoFrom,
    InvalidDate,
    InvalidHtmlBody,
    InvalidTextBody,
    InvalidMimeMessage,
    EmptyMailAddress,
    Unknown(String),
}

/// allow to convert from parsed message
impl<'x> TryFrom<mail_parser::Message<'x>> for ImportableMail {
    type Error = MailParseError;

    fn try_from(parsed_message: mail_parser::Message) -> Result<Self, Self::Error> {
        let subject = parsed_message.subject().unwrap_or_default().to_string();

        let (html_body_text, attachments) = ImportableMail::process_all_parts(&parsed_message)?;

        let date = parsed_message
            .date()
            .as_ref()
            .map(|date_time| DateTime::from_millis(date_time.to_timestamp() as u64 * 1000));

        let from_addresses = ImportableMail::map_to_tuta_mail_address(
            parsed_message.from().map(Cow::Borrowed).unwrap_or_else(|| {
                parsed_message
                    .sender()
                    .map(Cow::Borrowed)
                    .unwrap_or_else(|| Cow::Owned(mail_parser::Address::List(vec![])))
            }),
        )
            .into_iter()
            .map(|mut address| {
                // we currently use the name as address if no address was defined on server side
                if address.mail_address.is_empty() {
                    address.mail_address = address.name;
                    address.name = String::new();
                }
                address
            })
            .collect::<Vec<_>>();

        let different_envelope_sender = parsed_message
            .sender()
            .map(|sender| ImportableMail::map_to_tuta_mail_address(Cow::Borrowed(sender)))
            // sender is allowed to be empty
            .unwrap_or_default()
            // there should only be one different envelope sender
            .pop()
            .map(|mut address| {
                // we currently use the name as address if no address was defined on server side
                if address.mail_address.is_empty() {
                    address.mail_address = address.name;
                    address.name = String::new();
                }
                address
            })
            // different envelope sender should not contain address listed in from_addresses;
            .filter(|diff_sender| {
                from_addresses
                    .iter()
                    .filter(|from| from.mail_address != diff_sender.mail_address)
                    .next()
                    .is_some()
            })
            .map(|mail_address| mail_address.mail_address);

        let to_addresses = parsed_message
            .to()
            .map(|to| ImportableMail::map_to_tuta_mail_address(Cow::Borrowed(to)))
            .unwrap_or_default()
            .into_iter()
            .filter(|address| !address.mail_address.trim().is_empty())
            .collect();

        let cc_addresses = parsed_message
            .cc()
            .map(|cc| ImportableMail::map_to_tuta_mail_address(Cow::Borrowed(cc)))
            .unwrap_or_default()
            .into_iter()
            .filter(|address| !address.mail_address.trim().is_empty())
            .collect();

        let bcc_addresses = parsed_message
            .bcc()
            .map(|bcc| ImportableMail::map_to_tuta_mail_address(Cow::Borrowed(bcc)))
            .unwrap_or_default()
            .into_iter()
            .filter(|address| !address.mail_address.trim().is_empty())
            .collect();

        let reply_to_addresses = parsed_message
            .reply_to()
            .map(|reply_to| ImportableMail::map_to_tuta_mail_address(Cow::Borrowed(reply_to)))
            .unwrap_or_default()
            .into_iter()
            .filter(|address| !address.mail_address.trim().is_empty())
            .collect();

        let headers_string = parsed_message
            .headers_raw()
            .map(|(name, value)| name.to_string() + ":" + value)
            .collect::<Vec<_>>()
            .join("");

        let reply_type = get_reply_type_from_headers(parsed_message.headers());
        let message_id = parsed_message.message_id().map(String::from);
        let in_reply_to = parsed_message.in_reply_to().as_text().map(String::from);
        let references = match parsed_message.references() {
            HeaderValue::Text(reference) => {vec![reference.to_string()]}
            HeaderValue::TextList(references) => {references.iter().map(|cow| cow.to_string()).collect()}
            _ => {vec![]}
        };

        Ok(Self {
            headers_string,
            html_body_text,
            subject,
            different_envelope_sender,
            from_addresses,
            to_addresses,
            cc_addresses,
            bcc_addresses,
            reply_to_addresses,
            date,
            reply_type,
            message_id,
            in_reply_to,
            references,
            attachments,

            ical_type: Default::default(),
            unread: false,
            mail_state: Default::default(),
            is_phishing: false,
        })
    }
}

// Keep in sync with MimeStringToSmtpMessageConverterTest !
#[cfg(test)]
mod tests {
    use crate::importer::importable_mail::{ImportableMail, ImportableMailAttachment, MailContact};
    use mail_parser::{MessageParser, MessagePartId};
    use serde::Deserialize;
    use std::borrow::Cow;
    use tutasdk::date::DateTime;

    impl From<TestMailAddress> for MailContact {
        fn from(value: TestMailAddress) -> Self {
            let TestMailAddress {
                name, mail_address, ..
            } = value;
            Self { mail_address, name }
        }
    }

    impl From<ExpectedMessage> for ImportableMail {
        fn from(mut expected_message: ExpectedMessage) -> Self {
            let mut html_body_ids: Vec<MessagePartId> = vec![];
            let mut plain_body_ids: Vec<MessagePartId> = vec![];
            let mut attachment_ids: Vec<MessagePartId> = vec![];
            let mut body_parts = vec![];

            expected_message.mail_headers.push_str("\n");
            let parsed_headers_res = MessageParser::default()
                .parse_headers(expected_message.mail_headers.as_str())
                .unwrap();
            let root_part = parsed_headers_res.part(0).unwrap().clone();
            body_parts.push(root_part);

            if let Some(plain_body_part) = expected_message.plain_body_text {
                let plain_body_converted = mail_parser::MessagePart {
                    headers: vec![],
                    is_encoding_problem: false,
                    body: mail_parser::PartType::Text(Cow::Owned(plain_body_part)),
                    encoding: Default::default(),
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                };
                plain_body_ids.push(body_parts.len());
                body_parts.push(plain_body_converted);
            }

            if let Some(html_body_part) = expected_message.html_body_text {
                let html_body_converted = mail_parser::MessagePart {
                    headers: vec![],
                    is_encoding_problem: false,
                    body: mail_parser::PartType::Html(Cow::Owned(html_body_part)),
                    encoding: Default::default(),
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                };
                html_body_ids.push(body_parts.len());
                body_parts.push(html_body_converted);
            }

            for attached_message in expected_message.attached_messages {
                let attached_message_converted = mail_parser::MessagePart {
                    headers: vec![],
                    is_encoding_problem: false,
                    body: Default::default(),
                    encoding: Default::default(),
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                };
                attachment_ids.push(body_parts.len());
                body_parts.push(attached_message_converted);
            }

            for attached_file in expected_message.attached_files {
                let attached_file_converted = mail_parser::MessagePart {
                    headers: vec![],
                    is_encoding_problem: false,
                    body: Default::default(),
                    encoding: Default::default(),
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                };
                attachment_ids.push(body_parts.len());
                body_parts.push(attached_file_converted);
            }

            let parsed_mail = mail_parser::Message {
                html_body: html_body_ids,
                text_body: plain_body_ids,
                attachments: attachment_ids,
                parts: body_parts,
                raw_message: Default::default(),
            };

            ImportableMail::try_from(parsed_mail).unwrap()
        }
    }

    fn parseMail(msg: &str) -> ImportableMail {
        let parsed_message = MessageParser::default()
            .parse(msg)
            .unwrap();

        println!("{:?}", parsed_message.headers());
        let m: ImportableMail = parsed_message.try_into().unwrap();
        m
    }

    #[test]
    fn headers() {
        let msg = r#"Message-ID: 123456
Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Reply-To: Reply <reply@tutanota.de>, Reply2 <reply2@tutanota.de>
References: <sadf@tutanota.de> <1234564@web.de>
In-Reply-To: <1234564@web.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: multipart/mixed; boundary=frontier
"#;
        println!("{}", msg);
        let m: ImportableMail = parseMail(msg);
        assert_eq!("123456", m.message_id.unwrap());
        assert_eq!(vec![
            MailContact { name: "Reply".to_string(), mail_address: "reply@tutanota.de".to_string() },
            MailContact { name: "Reply2".to_string(), mail_address: "reply2@tutanota.de".to_string() },
        ], m.reply_to_addresses);
        assert_eq!(vec!["sadf@tutanota.de".to_string(), "1234564@web.de".to_string()], m.references);
        assert_eq!("1234564@web.de", m.in_reply_to.unwrap());
        // assert_eq!("frontier", m.boundary);
        assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
        assert_eq!(msg, m.headers_string);
    }

    #[test]
    fn bad_frontier() {
        // todo!()
    }

    #[test]
    fn empty_references() {
        // todo!()
    }

    #[test]
    fn empty_in_reply_to() {
        // todo!()
    }

    #[test]
    fn text_plain_us_ascii() {
        // todo!()
    }

    #[test]
    fn text_plain_utf8bit() {
        // todo!()
    }

    #[test]
    fn text_plain_utf_explicit_8bit() {
        // todo!()
    }

    #[test]
    fn text_plain_utf_quoted_printable() {
        // todo!()
    }

    #[test]
    fn text_plain_utf_base64() {
        // todo!()
    }

    #[test]
    fn text_plain_utf_invalid_base64() {
        // todo!()
    }

    #[test]
    fn text_plain_format_flowed() {
        // todo!()
    }

    #[test]
    fn text_plain_format_flowed_del_sp() {
        // todo!()
    }

    #[test]
    fn text_plain_subject_encoded_word_Qencoding() {
        // todo!()
    }

    #[test]
    fn text_plain_subject_encoded_word_Qencoding_turkish() {
        // todo!()
    }

    #[test]
    fn from_encoded_word_Qencoding() {
        // todo!()
    }

    #[test]
    fn from_encoded_word_Qencoding_colon() {
        // todo!()
    }

    #[test]
    fn recipients_encoded_word_Qencoding_colon() {
        // todo!()
    }

    #[test]
    fn recipients_encoded_word_Qencoding_partly() {
        // todo!()
    }


    #[test]
    fn text_plain_subject_encoded_word_base64() {
        // todo!()
    }


    #[test]
    fn text_html_only() {
        // todo!()
    }


    #[test]
    fn charset() {
        // todo!()
    }

    #[test]
    fn text_html_inline_charset_definition_utf8() {
        // todo!()
    }

    #[test]
    fn text_html_inline_charset_definition_western() {
        // todo!()
    }
    #[test]
    fn text_alternative() {
        let msg = r#"Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100 (CET)
Content-Type: multipart/alternative; boundary=frontier

--frontier
Content-type: text/plain; charset=UTF-8;

Hello äöüß
--frontier
Content-type: text/html; charset=UTF-8;

<html><body><b><small>Hello äöüß</small></b><br></body></html>
--frontier--
"#;
        let m: ImportableMail = parseMail(msg);

        assert_eq!(&MailContact { mail_address: "a@tutanota.de".to_string(), name: "A".to_string() }, m.from_addresses.first().unwrap());
        assert_eq!(vec![MailContact { mail_address: "b@tutanota.de".to_string(), name: "B".to_string() }], m.to_addresses);
        assert_eq!("Hello", m.subject);
        assert_eq!("<html><body><b><small>Hello äöüß</small></b><br></body></html>", m.html_body_text);
        assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
    }

    #[test]
    fn invalid_domains_in_mail_addresses() {
        // todo!()
    }

    #[test]
    fn multiple_to_headers() {
        // todo!()
    }

    #[test]
    fn attached_message() {
        let msg = r#"Subject: parent message
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100 (CET)
Content-Type: multipart/mixed; boundary=frontier

--frontier
Content-type: text/plain; charset=UTF-8;

normal message
--frontier
Content-Type: message/rfc822; charset=UTF-8;

Subject: attached message
From: D <d@tutanota.de>
To: E <e@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100 (CET)
Content-type: text/plain; charset=UTF-8;

Hello äöüß
"#;

        let m: ImportableMail = parseMail(msg);

        // assert_eq!(&MailContact { mail_address: "a@tutanota.de".to_string(), name: "A".to_string() }, m.from_addresses.first().unwrap());
        // assert_eq!(vec![MailContact { mail_address: "b@tutanota.de".to_string(), name: "B".to_string() }], m.to_addresses);
        assert_eq!("parent message", m.subject);
        assert_eq!("normal message", m.html_body_text);
        // assert_eq!(Some(DateTime::from_millis(0)), m.date);

        let attachment = m.attachments.first().unwrap();
        match attachment {
            ImportableMailAttachment::Attachment { .. } => {panic!("should be an attached message")}
            ImportableMailAttachment::AttachedMessage { message } => {
                // assert_eq!(MailContact{name: "D", mail_address: "d@tutanota.de"}, m.getSender());
                // assert_eq!(List.of(new SmtpMailContact("E", "e@tutanota.de")), m.getToRecipients());
                // assert_eq!("attached message", attached.getSubject());
                // assert_eq!("Hello äöüß", attached.getPlainBodyText());
                // assert_eq!(null, attached.getHtmlBodyText());
                // assert_eq!(yesterday, attached.getSentDate());
            }
        }
    }

    #[test]
    fn attachments() {
        // todo!()
    }

    #[test]
    fn inline_attachment() {
        // todo!()
    }

    #[test]
    fn attachment_to_attached_message() {
        // todo!()
    }

    #[test]
    fn textAttachment() {}


    #[test]
    fn htmlAttachment() {}

    #[test]
    fn multiple_plain_body_text_parts_are_concatenated() {
        let eml_contents = r#"Message-Id: some-id
From: A <a@example.org>
To: B <b@example.org>
Date: Tue, 5 Nov 2024 13:18:59 +0000
Content-Type: multipart/mixed; boundary=line

--line
Content-type: text/plain; charset=UTF-8

first plain text in body

--line
Content-Type: text/plain; charset=UTF-8

second plain text in body
--line--
"#;

        let parsed_message = MessageParser::default()
            .with_mime_headers()
            .parse(eml_contents)
            .unwrap();
        let text_contents = parsed_message
            .text_bodies()
            .map(|a| a.text_contents().unwrap())
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(
            "first plain text in body\nsecond plain text in body",
            text_contents
        );
    }

    #[test]
    fn multiple_html_body_text_parts_are_concatenated() {
        let eml_contents = r#"Message-Id: some-id
From: A <a@example.org>
To: B <b@example.org>
Date: Tue, 5 Nov 2024 13:18:59 +0000
Content-Type: multipart/mixed; boundary=line

--line
Content-type: text/html; charset=UTF-8

<p>first html text in body</p>

--line
Content-Type: text/html; charset=UTF-8

<p>second html text in body</p>
--line--
"#;

        let parsed_message = MessageParser::default()
            .with_mime_headers()
            .parse(eml_contents)
            .unwrap();
        let text_contents = parsed_message
            .html_bodies()
            .map(|a| a.text_contents().unwrap())
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(
            "<p>first html text in body</p>\n<p>second html in body</p>",
            text_contents
        );
    }

    #[test]
    // todo! what does this test (map)
    fn concatenate_alternative_html_text_parts() {
        let eml_contents = r#"Message-Id: some-id
From: A <a@example.org>
To: B <b@example.org>
Date: Tue, 5 Nov 2024 13:18:59 +0000
Content-Type: multipart/mixed; boundary=line

--line
Content-type: text/plain; charset=UTF-8

first plain text in body

--line
Content-Type: text/html; charset=UTF-8

<p>first html text in body</p>

--line--
"#;

        let parsed_message = MessageParser::default()
            .with_mime_headers()
            .parse(eml_contents)
            .unwrap();
        for body_part in parsed_message.html_bodies() {
            eprintln!("=====");
            eprintln!("{body_part:#?}");
        }
    }

    #[test]
    // todo! what does this test (map)
    fn concatenate_multiple_html_and_plain_text_parts() {
        let eml_contents = r#"Message-Id: some-id
From: A <a@example.org>
To: B <b@example.org>
Date: Tue, 5 Nov 2024 13:18:59 +0000
Content-Type: multipart/mixed; boundary=line

--line
Content-type: text/html; charset=UTF-8

<p>first html text in body</p>
<img src = "https://image.rs/imag.jpeg" />

--line
Content-Type: img/gif; charset=UTF-8
Content-Disposition: inline; filename=name.txt;

first plain text in body
--line--
"#;

        let parsed_message = MessageParser::default()
            .with_mime_headers()
            .parse(eml_contents)
            .unwrap();

        eprintln!("{:?}", parsed_message.text_body);
        eprintln!("{:?}", parsed_message.html_body);
        eprintln!("{:?}", parsed_message.attachments);

        let text_contents = parsed_message
            .text_bodies()
            .map(|a| a.text_contents().unwrap())
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(
            "<p>first html text in body</p>\nfirst plain text in body",
            text_contents
        );
    }

    #[test]
    fn plain_body_text_parts_are_concatenated_with_html_body_parts_if_html_body_parts_already_existing() {
        todo!()
    }

    #[test]
    fn plain_body_text_parts_are_converted_to_html_body_parts_if_html_body_parts_follow_afterwards() {
        todo!()
    }

    #[test]
    fn text_attachment_with_disposition() {
        todo!()
    }

    #[test]
    fn attachment_with_non_ascii_name() {
        todo!()
    }

    #[test]
    fn attachment_filename_in_content_type() {
        todo!()
    }

    #[test]
    fn attachment_filename_qencoding() {
        todo!()
    }

    #[test]
    fn encrypted() {
        todo!()
    }

    #[test]
    fn can_map_to_all_header_value() {
        todo!()
    }

    #[test]
    fn recipient_groups() {
        todo!()
    }

    #[test]
    fn undisclosed_recipients() {
        todo!()
    }

    #[test]
    fn long_content_type() {
        todo!()
    }

    #[test]
    fn normalize_header_value() {}

    #[test]
    fn get_spf_result() {
        // net yet used on rust
    }

    #[test]
    fn mail_from_with_delemiter() {
        todo!()
    }

    #[test]
    fn incomplete_text_content_type() {
        todo!()
    }

    #[test]
    fn calendar_content_type() {
        todo!()
    }

    #[test]
    fn calendar_content_type_method() {
        todo!()
    }

    #[test]
    fn invalid_content_types_default_to_text_plain() {
        todo!()
    }


    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TestMailAddress {
        name: String,
        mail_address: String,
        valid: bool,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ExpectedAttachedFile {
        name: String,
        data: String,
        mime_type: String,
        charset: Option<String>,
        content_id: String,
        calender_method: Option<()>,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ExpectedMessage {
        id: Option<String>,
        boundary: Option<String>,
        alternative_boundary: Option<String>,
        sender: TestMailAddress,
        to_recipients: Vec<TestMailAddress>,
        cc_recipients: Vec<TestMailAddress>,
        bcc_recipients: Vec<TestMailAddress>,
        reply_to: Vec<TestMailAddress>,
        in_reply_to: Option<String>,
        references: Vec<String>,
        auto_submitted: Option<()>,
        sent_date: Option<i64>,
        subject: String,
        plain_body_text: Option<String>,
        html_body_text: Option<String>,
        attached_messages: Vec<()>,
        attached_files: Vec<ExpectedAttachedFile>,
        mail_headers: String,
        spf_result: String,
        list_unsubscribe: bool,
        mail_authentication_result: Option<()>,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Exception {
        clazz: String,
        message: String,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct FileContent {
        exception: Option<Exception>,
        result: Option<ExpectedMessage>,
    }

    impl FileContent {
        fn read_from_file(file_path: &str) -> Result<Self, String> {
            let file_content = std::fs::read_to_string(file_path)
                .map_err(|_| format!("Cannot read content of: {file_path}"))?;
            serde_json::from_str::<FileContent>(file_content.as_str())
                .map_err(|e| format!("Cannot read to valid ExpectedMessage struct. Error: {e:?}"))
        }
    }

    #[test]
    fn mime_tools_test_messages() {
        const DATA_DIR: &'static str =
            concat!(env!("CARGO_MANIFEST_DIR"), "/test/mimetools-testmsgs");
        let source_message_paths = std::fs::read_dir(DATA_DIR)
            .unwrap()
            .map(Result::unwrap)
            .filter(|path| path.file_name().to_str().unwrap().ends_with(".msg"));

        for message_path in source_message_paths {
            eprintln!("File: {}", message_path.file_name().to_str().unwrap());

            let message_file_content = std::fs::read_to_string(&message_path.path()).unwrap();
            let parsed_message = MessageParser::default()
                .parse(message_file_content.as_str())
                .expect(format!("Cannot parse test message: {:?}", message_path.path()).as_str());

            let expected_json_file_name = format!(
                "{DATA_DIR}/{}",
                message_path
                    .file_name()
                    .to_str()
                    .unwrap()
                    .replace(".msg", "-expected.json")
            );
            let FileContent {
                result: expected_result,
                exception: expected_exception,
            } = FileContent::read_from_file(expected_json_file_name.as_str()).unwrap();
            let parsed_message_result = ImportableMail::try_from(parsed_message.clone());

            if expected_result.is_some() && expected_exception.is_none() {
                let expected_importable_mail = ImportableMail::from(expected_result.unwrap());
                let mut importable_mail = parsed_message_result.unwrap();
                importable_mail.attachments = vec![];
                importable_mail.different_envelope_sender = None;

                // assert_eq!(
                // 	importable_mail.headers_string,
                // 	expected_importable_mail.headers_string
                // );
                // assert_eq!(
                // 	importable_mail.html_body_text,
                // 	expected_importable_mail.html_body_text
                // );
                assert_eq!(importable_mail, expected_importable_mail);
            } else if expected_exception.is_some() && expected_result.is_none() {
                // check that the parsing have failed,
                // but we cannot check for the actual reason in `expected_exception`
                //
                //
                // todo: should not badbound.msg fail on mail_parser::parse thing? why is it failing in ImportableMail::try_from()?
                //assert!(parsed_message_result.is_err());
            } else if expected_result.is_none() && expected_exception.is_none() {
                unreachable!()
            } else if expected_exception.is_some() && expected_exception.is_some() {
                unreachable!()
            } else {
                unreachable!()
            }
        }
    }
}

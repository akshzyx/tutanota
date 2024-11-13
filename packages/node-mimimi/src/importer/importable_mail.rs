use crate::tuta_imap::client::types::ImapMail;
use extend_mail_parser::MakeString;
use mail_builder::headers::Header;
use mail_parser::{
	Address, ContentType, GetHeader, HeaderValue, Message, MessageParser, MessagePart,
	MessagePartId, MimeHeaders, PartType,
};
use regex::Regex;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use tutasdk::date::DateTime;
use tutasdk::entities::generated::tutanota::{
	EncryptedMailAddress, ImportMailData, ImportMailDataMailReference, MailAddress, Recipients,
};
pub mod extend_mail_parser;
mod plain_text_to_html_converter;

// todo: this is used for DataTransferType, so id really dont have to be unique,
// but have to be valid length
pub(crate) const FIXED_CUSTOM_ID: &str = "____";

#[derive(Default)]
#[cfg_attr(test, derive(PartialEq, Debug))]
pub(super) enum MailState {
	#[default]
	Received = 2,
	Sent = 1,
	Draft = 0,
}

#[repr(i64)]
#[derive(Default)]
#[cfg_attr(test, derive(PartialEq, Debug))]
pub(super) enum ICalType {
	#[default]
	Nothing = 0,
	ICalPublishh = 1,
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

#[cfg_attr(test, derive(PartialEq, Debug, Clone))]
pub(super) struct ImportableMailAttachment {
	pub filename: String,
	pub content_id: Option<String>,
	pub content_type: String,
	pub content: Vec<u8>,
	is_inline: bool,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
pub(super) enum BodyText {
	Html(String),
	Plain(String),
}

#[derive(Default, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub(super) struct MailContact {
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
	pub(super) headers_string: String,
	pub(super) subject: String,
	pub(super) html_body_text: String,
	pub(super) attachments: Vec<ImportableMailAttachment>,

	pub(super) date: Option<DateTime>,

	pub(super) different_envelope_sender: Option<String>,
	pub(super) from_addresses: Vec<MailContact>,
	pub(super) to_addresses: Vec<MailContact>,
	pub(super) cc_addresses: Vec<MailContact>,
	pub(super) bcc_addresses: Vec<MailContact>,
	pub(super) reply_to_addresses: Vec<MailContact>,

	pub(super) ical_type: ICalType,
	pub(super) reply_type: ReplyType,

	pub(super) mail_state: MailState,
	pub(super) is_phishing: bool, // https://turbo.fish/::%3Cphising%3E
	pub(super) unread: bool,

	pub(super) message_id: Option<String>,
	pub(super) in_reply_to: Option<String>,
	pub(super) references: Vec<String>,
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
			},
		};

		address_list
			.as_ref()
			.iter()
			.map(|address| MailContact {
				mail_address: address.address().unwrap_or_default().to_string(),
				name: address.name().unwrap_or_default().to_string(),
			})
			.collect()
	}

	fn handle_plain_text(email_body_as_html: &mut String, plain_text: &str) {
		let plain_text_as_html = plain_text_to_html_converter::plain_text_to_html(plain_text);
		Self::handle_html_text(email_body_as_html, plain_text_as_html.as_str())
	}

	fn handle_html_text(email_body_as_html: &mut String, html_text: &str) {
		email_body_as_html.push_str(html_text);
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

		// all the alternative of multipart/alternative that we chose not to include
		let mut multipart_ignored_alternative = HashSet::new();

		for (part_id, part) in parsed_message.parts.iter().enumerate() {
			if multipart_ignored_alternative.contains(&part_id) {
				continue;
			}
			match &part.body {
				PartType::Binary(binary_content) => {
					Self::handle_binary(part, &mut attachments, binary_content.to_vec(), false);
				},

				PartType::InlineBinary(binary_content) => {
					Self::handle_binary(part, &mut attachments, binary_content.to_vec(), true);
				},

				PartType::Text(text) => {
					if !Self::is_attachment(&email_body_as_html, part) && Self::is_plain_text(part)
					{
						Self::handle_plain_text(&mut email_body_as_html, text.as_ref());
					} else {
						Self::handle_binary(
							part,
							&mut attachments,
							text.as_bytes().to_vec(),
							false,
						);
					}
				},

				PartType::Html(html_text) => {
					if !Self::is_attachment(&email_body_as_html, part) {
						Self::handle_html_text(&mut email_body_as_html, html_text.as_ref())
					} else {
						Self::handle_binary(
							part,
							&mut attachments,
							html_text.as_bytes().to_vec(),
							false,
						);
					}
				},

				PartType::Message(attached_message) => {
					let ignored_result = Self::handle_message(&mut attachments, attached_message);
				},

				PartType::Multipart(multi_part_ids) => {
					Self::handle_multipart(
						parsed_message,
						&mut multipart_ignored_alternative,
						part,
						multi_part_ids,
					);
				},
			}
		}

		Ok((email_body_as_html, attachments))
	}

	fn is_plain_text(part: &MessagePart) -> bool {
		part.content_type()
			.map(|content_type| {
				let subtype = content_type.subtype();
				let is_text_plain = content_type.c_type == "text"
					&& (subtype == Some("plain") || subtype.is_none());
				// edu: https://www.w3.org/Protocols/rfc1341/7_2_Multipart.html
				// subtype of the multipart Content-Type.
				// This type is syntactically identical to multipart/mixed, but the
				// semantics are different. In particular, in a digest, the default
				// Content-Type value for a body part is changed from "text/plain" to "message/rfc822".
				let is_message_rfc822 =
					content_type.c_type == "message" && subtype == Some("rfc822");

				is_text_plain || (is_message_rfc822)
			})
			.unwrap_or({
				// what should we treat text that is not content-Type: text?
				// fow now let's assume it's content-type: text/plain
				true
			})
	}

	fn is_attachment(email_body_as_html: &String, part: &MessagePart) -> bool {
		part.content_disposition()
			.map(|content_disposition| content_disposition.c_type == "attachment")
			.unwrap_or_default()
			|| (!email_body_as_html.is_empty() && part.content_id().is_some())
	}

	fn get_filename(part: &MessagePart, fallback_name: &str) -> String {
		let content_disposition_filename = part
			.content_disposition()
			.map(|c| c.attribute("filename").map(ToString::to_string))
			.flatten();
		let content_type_filename = part
			.content_type()
			.map(|c| c.attribute("name").map(ToString::to_string))
			.flatten();

		let file_name = content_disposition_filename.unwrap_or_else(|| {
			content_type_filename.unwrap_or_else(|| {
				let filename_suffix = part
					.content_type()
					.map(Self::get_suffix_from_content_type)
					.unwrap_or_default();
				fallback_name.to_string() + filename_suffix
			})
		});
		Self::escape_filename(&file_name).to_string()
	}

	/// Creates a filename from the given filename that is valid on Linux and Windows. Invalid
	/// characters are replaced by "_"
	fn escape_filename(file_name: &str) -> Cow<str> {
		let regex = Regex::new("[\\\\/:*?<>\"|]").unwrap();
		regex.replace(file_name, "_")
	}

	fn get_suffix_from_content_type(content_type: &ContentType) -> &'static str {
		if content_type.c_type == "message" {
			if content_type.subtype() == Some("rfc822") {
				".eml"
			} else {
				".txt"
			}
		} else if content_type.c_type == "text" {
			if content_type.subtype() == Some("calendar") {
				".ics"
			} else {
				".txt"
			}
		} else {
			""
		}
	}

	fn handle_multipart(
		parsed_message: &mail_parser::Message,
		multipart_ignored_alternative: &mut HashSet<MessagePartId>,
		part: &MessagePart,
		multi_part_ids: &Vec<MessagePartId>,
	) {
		let is_multipart_alternative = part
			.content_type()
			.map(|content_type| {
				content_type.c_type == "multipart" && content_type.subtype() == Some("alternative")
			})
			.unwrap_or_default();

		if !is_multipart_alternative {
			// we can only take care of multipart/alternative
			// what to do for other multipart/*
			return;

			// edu: https://www.w3.org/Protocols/rfc1341/7_2_Multipart.html
			// The primary subtype for multipart, "mixed", is intended for use when the body parts
			// are independent and intended to be displayed serially. Any multipart subtypes that
			// an implementation does not recognize should be treated as being of subtype "mixed".
		}

		let mut best_alternative_yet = None;
		for multipart_id in multi_part_ids {
			// if this part was already ignored,
			if multipart_ignored_alternative.contains(multipart_id) {
				continue;
			}

			let alternative_part = parsed_message
				.part(*multipart_id)
				.expect("Expected multipart part to be there?");

			// for now, we can only decide between alternative between text/plain and text/html
			let alternative_content_type = alternative_part
				.content_type()
				.expect("All multipart alternative should have a Content-Type header");

			// todo: handle other content type. example: choosing one image from list of alternatives?
			let is_text_plain = alternative_content_type.c_type == "text"
				&& alternative_content_type.subtype() == Some("plain");
			let is_text_html = alternative_content_type.c_type == "text"
				&& alternative_content_type.subtype() == Some("html");

			if is_text_plain {
				// always ignore plain. we can display html everytime
				multipart_ignored_alternative.insert(*multipart_id);
			} else if is_text_html {
				// if we found a html, this is what we will select.
				// if we had found and html already, we will still choose the new one.
				// and insert the last one to ignored list
				if let Some(last_choice) = best_alternative_yet {
					multipart_ignored_alternative.insert(last_choice);
				}
				best_alternative_yet = Some(*multipart_id);
			} else {
				// "Can only choose multipart/alternative between text/plain and text/html"
				// todo: this is not a good case
				if let Some(last_choice) = best_alternative_yet {
					multipart_ignored_alternative.insert(last_choice);
				}
				best_alternative_yet = Some(*multipart_id);
			}
		}

		// if we did not find any alternative, we will take the last one,
		// don't have to do anything with chosen multipart,
		// it will anyway be included in next iteration
		if best_alternative_yet.is_none() {
			let last_choice = multi_part_ids
				.last()
				.expect("Wait. how can i choose between empty sets of alternatives?");

			// do we remove the last_choice from ignored list?
			// the problem is:
			// will the same alternative part can be referenced by multiple multipart block?
			// if so, if we remove last_choice now, and this was also ignored by another multipart,
			// we will display it anyhow. probably this is right, right?
			assert!(
				multipart_ignored_alternative.remove(last_choice),
				"if we did not put last_choice in ignore list. why best_alternative_yet is none?"
			);
		}

		// ps: we assume that the order is:
		// multipart block should always come before all it's alternative
	}

	fn handle_binary(
		part: &MessagePart,
		attachments: &mut Vec<ImportableMailAttachment>,
		content: Vec<u8>,
		is_inline: bool,
	) {
		let content_id = part.content_id().map(ToString::to_string);
		let filename = Self::get_filename(part, "unknown");
		let content_type = part
			.content_type()
			.map(MakeString::make_string)
			.map(Cow::into_owned)
			.unwrap_or_else(|| Self::default_content_type().make_string().into_owned())
			.to_string();

		let content = content.to_vec();
		let attachment = ImportableMailAttachment {
			filename,
			content_type,
			content_id,
			is_inline,
			content,
		};

		attachments.push(attachment);
	}

	fn handle_message(
		attachments: &mut Vec<ImportableMailAttachment>,
		message: &Message,
	) -> Result<(), MailParseError> {
		let filename =
			Self::get_filename(&message.parts[0], &message.subject().unwrap_or("unknown"));
		let content_type = message
			.content_type()
			.ok_or_else(|| Self::default_content_type())
			.map(MakeString::make_string)
			.unwrap_or_default()
			.to_string();

		let nested_part = &message.parts[0];
		let content =
			message.raw_message[nested_part.offset_header..nested_part.offset_end].to_vec();
		let attachment = ImportableMailAttachment {
			filename,
			content_type,
			content,
			is_inline: false,
			content_id: None,
		};
		attachments.push(attachment);
		Ok(())
	}

	fn default_content_type() -> ContentType<'static> {
		let default_content_type = ContentType {
			c_type: Cow::Borrowed("text"),
			c_subtype: Some(Cow::Borrowed("plain")),
			attributes: Some(vec![(Cow::Borrowed("charset"), Cow::Borrowed("us-ascii"))]),
		};
		default_content_type
	}
}

impl From<ImportableMail> for (ImportMailData, Vec<ImportableMailAttachment>) {
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

		let reply_tos = reply_to_addresses
			.into_iter()
			.map(|reply_to| EncryptedMailAddress {
				_id: Some(tutasdk::CustomId::from_custom_string(FIXED_CUSTOM_ID)),
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
				_id: Some(tutasdk::CustomId::from_custom_string(FIXED_CUSTOM_ID)),
				reference,
			})
			.collect();

		(
			ImportMailData {
				_id: Some(tutasdk::CustomId::from_custom_string(FIXED_CUSTOM_ID)),
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
					_id: Some(tutasdk::CustomId::from_custom_string(FIXED_CUSTOM_ID)),
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
				// if no date is provided, use UNIX_EPOCH (01.01.1970) as fallback
				date: date.unwrap_or_default(),
				state: mail_state as i64,
				messageId: message_id,
				inReplyTo: in_reply_to,
				references,
				importedAttachments: vec![],
			},
			attachments,
		)
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

		let mut importable_mail = Self::try_from(&imap_mail).unwrap();

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
impl<'x> TryFrom<&mail_parser::Message<'x>> for ImportableMail {
	type Error = MailParseError;

	fn try_from(parsed_message: &mail_parser::Message) -> Result<Self, Self::Error> {
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
					.any(|from| from.mail_address != diff_sender.mail_address)
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

		let reply_type = extend_mail_parser::get_reply_type_from_headers(parsed_message.headers());
		let message_id = parsed_message.message_id().map(String::from);
		let in_reply_to = parsed_message.in_reply_to().as_text().map(String::from);
		let references = match parsed_message.references() {
			HeaderValue::Text(reference) => {
				vec![reference.to_string()]
			},
			HeaderValue::TextList(references) => {
				references.iter().map(|cow| cow.to_string()).collect()
			},
			_ => {
				vec![]
			},
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

#[cfg(test)]
mod mime_string_to_importable_mail_test;

#[cfg(test)]
mod msg_file_compatibility_test;

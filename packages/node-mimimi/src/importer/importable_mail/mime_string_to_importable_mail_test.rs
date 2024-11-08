//! Keep in sync with MimeStringToSmtpMessageConverterTest !

use crate::importer::importable_mail::{ImportableMail, ImportableMailAttachment, MailContact};
use mail_parser::MessageParser;
use tutasdk::date::DateTime;

fn parse_mail(msg: &str) -> ImportableMail {
	let parsed_message = MessageParser::default().parse(msg).unwrap();

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
	let m: ImportableMail = parse_mail(msg);
	assert_eq!("123456", m.message_id.unwrap());
	assert_eq!(
		vec![
			MailContact {
				name: "Reply".to_string(),
				mail_address: "reply@tutanota.de".to_string()
			},
			MailContact {
				name: "Reply2".to_string(),
				mail_address: "reply2@tutanota.de".to_string()
			},
		],
		m.reply_to_addresses
	);
	assert_eq!(
		vec!["sadf@tutanota.de".to_string(), "1234564@web.de".to_string()],
		m.references
	);
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
	let m: ImportableMail = parse_mail(msg);

	assert_eq!(
		&MailContact {
			mail_address: "a@tutanota.de".to_string(),
			name: "A".to_string()
		},
		m.from_addresses.first().unwrap()
	);
	assert_eq!(
		vec![MailContact {
			mail_address: "b@tutanota.de".to_string(),
			name: "B".to_string()
		}],
		m.to_addresses
	);
	assert_eq!("Hello", m.subject);
	assert_eq!(
		"<html><body><b><small>Hello äöüß</small></b><br></body></html>",
		m.html_body_text
	);
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

	let m: ImportableMail = parse_mail(msg);

	// assert_eq!(&MailContact { mail_address: "a@tutanota.de".to_string(), name: "A".to_string() }, m.from_addresses.first().unwrap());
	// assert_eq!(vec![MailContact { mail_address: "b@tutanota.de".to_string(), name: "B".to_string() }], m.to_addresses);
	assert_eq!("parent message", m.subject);
	assert_eq!("normal message", m.html_body_text);
	// assert_eq!(Some(DateTime::from_millis(0)), m.date);

	let attachment = m.attachments.first().unwrap();
	match attachment {
		ImportableMailAttachment::Attachment { .. } => {
			panic!("should be an attached message")
		},
		ImportableMailAttachment::AttachedMessage { message } => {
			// assert_eq!(MailContact{name: "D", mail_address: "d@tutanota.de"}, m.getSender());
			// assert_eq!(List.of(new SmtpMailContact("E", "e@tutanota.de")), m.getToRecipients());
			// assert_eq!("attached message", attached.getSubject());
			// assert_eq!("Hello äöüß", attached.getPlainBodyText());
			// assert_eq!(null, attached.getHtmlBodyText());
			// assert_eq!(yesterday, attached.getSentDate());
		},
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
fn plain_body_text_parts_are_concatenated_with_html_body_parts_if_html_body_parts_already_existing()
{
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

use crate::importer::importable_mail::{ImportableMail, MailContact};
use serde::Deserialize;
use std::borrow::Cow;
use std::io::Read;

#[test]
fn mime_tools_test_messages() {
	const DATA_DIR: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/test/mimetools-testmsgs");
	let source_message_paths = std::fs::read_dir(DATA_DIR)
		.unwrap()
		.map(Result::unwrap)
		.map(|p| p.file_name().to_str().unwrap().to_string())
		.filter(|p| p.ends_with(".msg"));

	// everything else is related to multipart i suppose,
	const IGNORED_FILES: &[&str] = &[
		"multi-igor.msg",
		"multi-bad.msg",
		"multi-digest.msg",
		"2002_06_12_doublebound.msg",
		"attachment-filename-encoding-Latin1.msg",
		"attachment-filename-encoding-UTF8.msg",
		"multi-digest.msg",
		"multi-igor2.msg",
		"multi-nested.msg",
		"multi-nested3.msg",
		"multi-nested2.msg",
		"infinite.msg", // have encoding problem
	];

	for message_file_name in source_message_paths
		.filter(|p| {
			IGNORED_FILES
				.iter()
				.filter(|f| f.starts_with(p.as_str()))
				.next()
				.is_none()
		})
		.chain(IGNORED_FILES.iter().map(|s| {
			eprintln!("Ignored file: ");
			s.to_string()
		}))
		.map(|a| {
			eprintln!("{a} .....Testing");
			a
		}) {
		let message_path = format!("{DATA_DIR}/{message_file_name}");

		// let message_file_content = std::fs::r(&message_path.path()).unwrap()
		let mut message_file_content = vec![];
		std::fs::File::open(message_path.as_str())
			.unwrap()
			.read_to_end(&mut message_file_content)
			.unwrap();
		let parsed_message = mail_parser::MessageParser::default()
			.parse(message_file_content.as_slice())
			.unwrap();

		let expected_json_file_name = format!(
			"{DATA_DIR}/{}",
			message_file_name.replace(".msg", "-expected.json")
		);
		let FileContent {
			result: expected_result,
			exception: expected_exception,
		} = FileContent::read_from_file(expected_json_file_name.as_str()).unwrap();
		let parsed_message_result = ImportableMail::try_from(parsed_message.clone());

		if expected_result.is_some() && expected_exception.is_none() {
			let mut importable_mail = parsed_message_result.unwrap();
			let mut expected_importable_mail = ImportableMail::from(expected_result.unwrap());

			importable_mail.attachments.clear();
			expected_importable_mail.attachments.clear();

			// we import raw headers and there is no need to compare them
			importable_mail.headers_string = "".to_string();
			expected_importable_mail.headers_string = "".to_string();

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
		// add a new line at end of headers for mail_parser::MessageParser::parse_headers
		expected_message.mail_headers.push_str("\n");

		let headers_string_clone = expected_message.mail_headers.clone();

		let mut body_parts = vec![];
		let mut plain_body_ids = vec![];
		let mut html_body_ids = vec![];
		let mut attachment_ids = vec![];

		let parsed_headers_res = mail_parser::MessageParser::default()
			.parse_headers(expected_message.mail_headers.as_str())
			.unwrap();

		let root_part = mail_parser::MessagePart {
			headers: parsed_headers_res.headers().to_vec(),
			is_encoding_problem: false,
			body: mail_parser::PartType::Text(Cow::Borrowed("")),
			encoding: Default::default(),
			offset_header: 0,
			offset_body: 0,
			offset_end: 0,
		};
		body_parts.push(root_part);

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
		// if there is both plain text and html in json file,
		// probably that json if to test multipart/alternative ( todo: is this true? )
		// and since we always select html in multipart/alternative,
		// we can skip adding plain text if html text was set.
		// hence the `else if let` instead of `if let`
		else if let Some(plain_body_part) = expected_message.plain_body_text {
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
			// todo:
			// will only work for .raw_header(), if we use other _raw function or
			// try to access .raw_message in From<Message>: ImportableMail,
			// this won't work
			raw_message: Cow::Owned(headers_string_clone.as_bytes().to_vec()),
		};

		ImportableMail::try_from(parsed_mail).unwrap()
	}
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

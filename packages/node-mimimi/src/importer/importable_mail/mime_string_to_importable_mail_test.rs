//! Keep in sync with MimeStringToSmtpMessageConverterTest !

use crate::importer::importable_mail::{ImportableMail, ImportableMailAttachment, MailContact};
use mail_parser::{MessageParser, MimeHeaders};
use tutasdk::date::DateTime;

fn parse_mail(msg: &str) -> ImportableMail {
    MessageParser::default()
        .parse(msg)
        .unwrap()
        .try_into()
        .unwrap()
}

// to be able to convert any (str/string, str/string).into() => MailContact
impl<N, A> From<(N, A)> for MailContact
where
    N: ToString,
    A: ToString,
{
    fn from((name, address): (N, A)) -> Self {
        Self {
            mail_address: address.to_string(),
            name: name.to_string(),
        }
    }
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
    let m = parse_mail(msg);
    assert_eq!("123456", m.message_id.unwrap());
    assert_eq!(
		m.reply_to_addresses,
		vec![
			("Reply", "reply@tutanota.de").into(),
			("Reply2", "reply2@tutanota.de").into(),
		],
	);
    assert_eq!(
		m.references,
		vec!["sadf@tutanota.de".to_string(), "1234564@web.de".to_string()],
	);
    assert_eq!(Some("1234564@web.de".to_string()), m.in_reply_to);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
    assert_eq!(msg, m.headers_string);
}

#[test]
fn bad_frontier() {
    let msg = "Content-Type: multipart/mixed; boundary=komma;ist;nicht;erlaubt\n";
    let parsed_message = MessageParser::default().parse(msg).unwrap();
    let attributes = parsed_message
        .content_type()
        .unwrap()
        .attributes
        .as_ref()
        .unwrap();
    assert_eq!(attributes.as_slice(), [("boundary".into(), "komma".into())]);
}

#[test]
fn empty_references() {
    let msg = "Subject: Hello";
    let m = parse_mail(msg);
    assert!(m.references.is_empty());
}

#[test]
fn empty_in_reply_to() {
    let msg = "Subject: Hello";
    let m = parse_mail(msg);
    assert_eq!(None, m.in_reply_to);
}

#[test]
fn text_plain_us_ascii_7bit() {
    let msg = r##"Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100

US-ASCII:  !"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\]^_`abcdefghijklmnopqrstuvwxyz{|}~"##;
    let m = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(m.subject, "Hello",);
    assert_eq!("US-ASCII:  !\"#$%&amp;'()*+,-./0123456789:;&lt;=&gt;?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~", m.html_body_text);
    assert_eq!(m.date, Some(DateTime::from_millis(1730991244000)));
}

#[test]
fn text_plain_utf8bit() {
    let msg = r##"Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: text/plain; charset=UTF-8

Tutanota: äüöß€*#\{³|@"##;
    let m = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("b", "b@tutanota.de").into()]);
    assert_eq!("Hello", m.subject);
    assert_eq!("Tutanota: äüöß€*#\\{³|@", m.html_body_text);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
}

#[test]
fn text_plain_utf_explicit_8bit() {
    let msg = r##"Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: text/plain; charset=UTF-8
Content-Transfer-Encoding: 8bit

Tutanota: äüöß€*#\\{³|@"##;

    let m = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("b", "b@tutanota.de").into()]);
    assert_eq!("Hello", m.subject);
    assert_eq!("Tutanota: äüöß€*#\\{³|@", m.html_body_text);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
}

#[test]
fn text_plain_utf_quoted_printable() {
    let msg = r##"Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: text/plain; charset=UTF-8
Content-Transfer-Encoding: quoted-printable

Tutanota: =C3=A4=C3=BC=C3=B6=C3=9F=E2=82=AC*#\{=C2=B3|@"##;
    let m = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!("Tutanota: äüöß€*#\\{³|@", m.html_body_text);
    assert_eq!("Hello", m.subject);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
}

#[test]
fn text_plain_utf_base64() {
    let msg = r##"Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: text/plain; charset=UTF-8
Content-Transfer-Encoding: base64

VHV0YW5vdGE6IMOkw7zDtsOf4oKsKiNce8KzfEA="##;
    let m = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!("Hello", m.subject);
    assert_eq!("Tutanota: äüöß€*#\\{³|@", m.html_body_text);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
}

#[test]
fn text_plain_utf_invalid_base64() {
    let msg = r##"Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: text/plain; charset=UTF-8
Content-Transfer-Encoding: base64

VHV0YW5vdGE6IMOkw7zDtsOf4oKsKiNce8KzfEA"##; // skip the padding "=" to force an exception
    let m = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!("Hello", m.subject);
    assert_eq!("Tutanota: äüöß€*#\\{³", m.html_body_text);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
}

#[test]
fn text_plain_format_flowed() {
    let msg = r#"Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: text/plain; charset=UTF-8; format=flowed
				
Tutanota: ist so toll und diese Zeile wird länger, deshalb gibt es \r\neinen soft-break!!!!!\r\nVor dieser Zeile gibt es keinen soft break, sondern einen richtigen!"#;
    let m = parse_mail(msg);

    assert_eq!("Tutanota: ist so toll und diese Zeile wird länger, deshalb gibt es einen soft-break!!!!!\r\nVor dieser Zeile gibt es keinen soft break, sondern einen richtigen!", m.html_body_text);
}

#[test]
fn text_plain_format_flowed_del_sp() {
    let msg = r#"From: A <a@tutanota.de>
To: B <b@tutanota.de>
Subject: Hello
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: text/plain; charset=UTF-8; format=flowed; DelSp=yes

Tutanota: ist so toll und diese Zeile wird länger, deshalb gibt es \r\neinen soft-break!!!!!\r\nVor dieser Zeile gibt es keinen soft break, sondern einen richtigen!"#;
    let m = parse_mail(msg);
    assert_eq!(
        "Tutanota: ist so toll und diese Zeile wird länger, deshalb gibt eseinen soft-break!!!!!\r\nVor dieser Zeile gibt es keinen soft break, sondern einen richtigen!",
        m.html_body_text);
}

#[test]
fn text_plain_subject_encoded_word_qencoding() {
    let msg = r#"Subject: Hello =?ISO-8859-1?Q?=E4=F6=FC=DF?=abc
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
"#;
    let m = parse_mail(msg);
    assert_eq!("Hello äöüßabc", m.subject);
}

#[test]
fn text_plain_subject_encoded_word_qencoding_turkish() {
    let msg = r#"Subject: =?iso-8859-9?Q?Paracard Hesap =D6zeti?=
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
"#;
    let m = parse_mail(msg);
    assert_eq!("Paracard Hesap Özeti", m.subject);
}

#[test]
fn from_encoded_word_qencoding() {
    let msg = r#"Subject: =?utf-8?Q?=D0=9E=D0=B1=D1=8A=D0=B5=D0=B4=D0=B8=D0=BD=D0=B5=D0?==?utf-8?Q?=BD=D0=BD=D1=8B=D0=B5?==?utf-8?Q?_=D0=B4=D0=B5=D0=BC=D0=BE=D0=BA=D1=80=D0=B0=D1=82=D1=8B?=
From: =?utf-8?Q?=D0=9E=D0=B1=D1=8A=D0=B5=D0=B4=D0=B8=D0=BD=D0=B5=D0?==?utf-8?Q?=BD=D0=BD=D1=8B=D0=B5?==?utf-8?Q?_=D0=B4=D0=B5=D0=BC=D0=BE=D0=BA=D1=80=D0=B0=D1=82=D1=8B?=<team@od.spb.ru>
To: =?utf-8?Q?=D0=9E=D0=B1=D1=8A=D0=B5=D0=B4=D0=B8=D0=BD=D0=B5=D0?==?utf-8?Q?=BD=D0=BD=D1=8B=D0=B5?==?utf-8?Q?_=D0=B4=D0=B5=D0=BC=D0=BE=D0=BA=D1=80=D0=B0=D1=82=D1=8B?=<team@od.spb.ru>
Date: Thu, 7 Nov 2024 15:54:04 +0100
"#;
    let m = parse_mail(msg);
    assert_eq!("Объединенные демократы", m.subject);
    assert_eq!("Объединенные демократы", m.from_addresses[0].name);
    assert_eq!("Объединенные демократы", m.to_addresses[0].name);
}

#[test]
fn from_encoded_word_qencoding_colon() {
    let msg = r#"Subject: Hi
From: =?utf-8?Q?=D0=9B=D0=B8=D1=82=D1=80=D0=B5=D1=81=3A=20=D0=A1=D0=B0=D0=BC=D0=B8=D0=B7=D0=B4=D0=B0=D1=82?= <mail@selfpub.ru>
"#;
    let m = parse_mail(msg);
    assert_eq!(
        m.from_addresses[0],
        ("Литрес: Самиздат", "mail@selfpub.ru").into()
    );
}

#[test]
fn recipients_encoded_word_qencoding_colon() {
    let msg = "To: =?utf-8?Q?=D0=9B=D0=B8=D1=82=D1=80=D0=B5=D1=81=3A=20=D0=A1=D0=B0=D0=BC=D0=B8=D0=B7=D0=B4=D0=B0=D1=82?= <mail@selfpub.ru>\n";
    let m = parse_mail(msg);
    assert_eq!(
        m.from_addresses[0],
        ("Литрес: Самиздат", "mail@selfpub.ru").into()
    );
}

#[test]
fn recipients_encoded_word_qencoding_partly() {
    let msg = "To: =?ISO-8859-1?Q?Andr=E9?= Pirard <PIRARD@vm1.ulg.ac.be>\n";
    let m = parse_mail(msg);
    assert_eq!(
        m.from_addresses[0],
        ("André Pirard", "PIRARD@vm1.ulg.ac.be").into()
    );
}

#[test]
fn text_plain_subject_encoded_word_base64() {
    let msg = r#"Subject: =?utf-8?B?SGVsbG8gw6TDtsO8w58=?=
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
"#;
    let m = parse_mail(msg);
    assert_eq!("Hello äöüß", m.subject);
}

#[test]
fn text_html_only() {
    let msg = r#"Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: text/html; charset=UTF-8

<html><body><b><small>Hello äöüß</small></b><br></body></html>"#;
    let m = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
    assert_eq!(
        "<html><body><b><small>Hello äöüß</small></b><br></body></html>",
        m.html_body_text
    );
}

#[test]
fn charset() {
    // todo!()
}

#[test]
fn text_html_inline_charset_definition_utf8() {
    let msg = r#"Content-type: text/html
Content-Transfer-Encoding: 8bit

<html><head><meta charset=\"utf-8\"></head><body><p>Благодарим Ви</p></body></html>"#;
    let m = parse_mail(msg);

    assert_eq!(
        "<html><head><meta charset=\"utf-8\"></head><body><p>Благодарим Ви</p></body></html>",
        m.html_body_text
    );
}

#[test]
fn text_html_inline_charset_definition_western() {
    let msg = r#"Content-type: text/html
Content-Transfer-Encoding: base64

PGh0bWw+PGhlYWQ+PG1ldGEgY2hhcnNldD0iSVNPLTg4NTktMTUiPjwvaGVhZD48Ym9keT48cD6kIPbkPC9wPjwvYm9keT48L2h0bWw+"#;
    let m = parse_mail(msg);
    assert_eq!(
        "<html><head><meta charset=\"ISO-8859-15\"></head><body><p>€ öä</p></body></html>",
        m.html_body_text
    );
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

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
    assert_eq!("Hello", m.subject);
    assert_eq!(
        "<html><body><b><small>Hello äöüß</small></b><br></body></html>",
        m.html_body_text
    );
}

#[test]
fn invalid_domains_in_mail_addresses() {
    let msg = r#"Subject: Hello
From: A <a@tutanota.de>
To: B <b@a.example>, C <c@c.com>, D <d@d.invalid>
"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(
        m.to_addresses,
        vec![
            ("B", "b@a.example").into(),
            ("C", "c@c.com").into(),
            ("D", "d@d.invalid").into()
        ]
    );
}

#[test]
fn multiple_to_headers() {
    let msg = r#"Subject: Hello
From: A <a@tutanota.de>
To: B <b@b.org>, C <c@c.com>
To: D <d@d.net>
"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(
        m.to_addresses,
        vec![
            ("B", "b@b.org").into(),
            ("C", "c@c.com").into(),
            ("D", "d@d.net").into()
        ]
    );
}

#[test]
fn attached_message() {
    let msg = r#"Subject: parent message
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: multipart/mixed; boundary=frontier

--frontier
Content-type: text/plain; charset=UTF-8;

normal message
--frontier
Content-Type: message/rfc822; charset=UTF-8;

Subject: attached message
From: D <d@tutanota.de>
To: E <e@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: text/plain; charset=UTF-8;


Hello äöüß
"#;

    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(m.subject, "parent message");
    assert_eq!(m.html_body_text, "normal message");
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);

    assert_eq!(m.attachments.len(), 1);
    for attachment in m.attachments {
        match attachment {
            ImportableMailAttachment::Attachment { .. } => {
                // todo:
                // should we have got the attachment here or in AttachedMessage?
            }
            ImportableMailAttachment::AttachedMessage { message: attached } => {
                assert_eq!(attached.from_addresses, vec![("D", "d@tutanota.de").into()]);
                assert_eq!(attached.to_addresses, vec![("E", "e@tutanota.de").into()]);
                assert_eq!(attached.subject, "attached message");
                assert_eq!(attached.html_body_text, "Hello äöüß");
                assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date)
            }
        }
    }

//     		assertEquals(new SmtpMailContact("A", "a@tutanota.de"), m.getSender());
    // 		assertEquals(List.of(new SmtpMailContact("B", "b@tutanota.de")), m.getToRecipients());
    // 		assertEquals("parent message", m.getSubject());
    // 		assertEquals("normal message", m.getPlainBodyText());
    // 		assertEquals(null, m.getHtmlBodyText());
    // 		assertEquals(date, m.getSentDate());
    //
    // 		assertEquals(0, m.getAttachedMessages().size());
    // 		assertEquals(1, m.getAttachedFiles().size());
    // 		SmtpAttachment attachement = m.getAttachedFiles().get(0);
    // 		SmtpMessage attached = mimeStringToSmtpMessageConverter.mimeToSmtpMessage(mimeStringToSmtpMessageConverter.dataToMimeMessage(attachement.getData()),
    // 				null);
    //
    // 		assertEquals(new SmtpMailContact("D", "d@tutanota.de"), attached.getSender());
    // 		assertEquals(List.of(new SmtpMailContact("E", "e@tutanota.de")), attached.getToRecipients());
    // 		assertEquals("attached message", attached.getSubject());
    // 		assertEquals("Hello äöüß", attached.getPlainBodyText());
    // 		assertEquals(null, attached.getHtmlBodyText());
    // 		assertEquals(yesterday, attached.getSentDate());
}

#[test]
fn attachments() {
    let msg = r#"Subject: multiple attachments
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: multipart/mixed; boundary=frontier

--frontier
Content-type: text/plain; charset=UTF-8;

Hello äöüß
--frontier
Content-type: application/octet-stream;
Content-Transfer-Encoding: base64
Content-Disposition: attachment; filename=a1.txt;

Zmlyc3QgYXR0YWNobWVudA==
--frontier
Content-type: application/pdf;
Content-Transfer-Encoding: base64
Content-Disposition: attachment; filename=a2.pdf;

c2Vjb25kIGF0dGFjaG1lbnQ=
--frontier
Content-Transfer-Encoding: base64
Content-Disposition: attachment; filename=withoutContentType.pdf;

c2Vjb25kIGF0dGFjaG1lbnQ=
--frontier--
"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(m.subject, "multiple attachments");

    assert_eq!(m.attachments.len(), 3);
    todo!()

//     		assertEquals("a@tutanota.de", m.getSender().getMailAddress());
    // 		assertEquals("A", m.getSender().getName());
    // 		assertEquals(1, m.getToRecipients().size());
    // 		assertEquals("b@tutanota.de", m.getToRecipients().get(0).getMailAddress());
    // 		assertEquals("B", m.getToRecipients().get(0).getName());
    // 		assertEquals("multiple attachments", m.getSubject());
    // 		assertEquals("Hello äöüß", m.getPlainBodyText());
    // 		assertEquals(null, m.getHtmlBodyText());
    // 		assertEquals(date, m.getSentDate());
    //
    // 		assertEquals(3, m.getAttachedFiles().size());
    // 		SmtpAttachment a1 = m.getAttachedFiles().get(0);
    // 		SmtpAttachment a2 = m.getAttachedFiles().get(1);
    // 		SmtpAttachment a3 = m.getAttachedFiles().get(2);
    //
    // 		assertEquals("a1.txt", a1.getName());
    // 		assertArrayEquals(EncodingConverter.base64ToBytes("Zmlyc3QgYXR0YWNobWVudA=="), a1.getData());
    // 		assertEquals("application/octet-stream", a1.getMimeType());
    // 		assertNull(a1.getCharset());
    //
    // 		assertEquals("a2.pdf", a2.getName());
    // 		assertArrayEquals(EncodingConverter.base64ToBytes("c2Vjb25kIGF0dGFjaG1lbnQ="), a2.getData());
    // 		assertEquals("application/pdf", a2.getMimeType());
    // 		assertNull(a1.getCharset());
    //
    // 		assertEquals("withoutContentType.pdf", a3.getName());
    // 		assertArrayEquals(EncodingConverter.base64ToBytes("c2Vjb25kIGF0dGFjaG1lbnQ="), a3.getData());
    // 		assertEquals("text/plain", a3.getMimeType());
    // 		assertNull(a1.getCharset());
}

#[test]
fn inline_attachment() {
    let msg = r#"Subject: inline attachment
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: multipart/mixed; boundary=frontier

--frontier
Content-type: text/html; charset=UTF-8;

<html><body><img src="cid:123@tutanota.de"/></body></html>
--frontier
Content-type: application/octet-stream;
Content-Transfer-Encoding: base64
Content-Disposition: inline; filename=a1.png;
Content-ID: <123@tutanota.de>;

Zmlyc3QgYXR0YWNobWVudA==
--frontier--
"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);

    todo!()

//     		assertEquals(1, m.getAttachedFiles().size());
    // 		SmtpAttachment a1 = m.getAttachedFiles().get(0);
    //
    // 		assertEquals("a1.png", a1.getName());
    // 		assertArrayEquals(EncodingConverter.base64ToBytes("Zmlyc3QgYXR0YWNobWVudA=="), a1.getData());
    // 		assertEquals("application/octet-stream", a1.getMimeType());
    // 		assertEquals("123@tutanota.de", a1.getContentId());
    // 		assertNull(a1.getCharset());
}

#[test]
fn attachment_to_attached_message() {
    let msg = r#"Subject: parent message
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: message/rfc822; charset=UTF-8;

Subject: attached message
From: D <d@tutanota.de>
To: E <e@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: application/octet-stream;
Content-Transfer-Encoding: base64
Content-Disposition: attachment; filename=indirectly_attached.txt;

Zmlyc3QgYXR0YWNobWVudA==
"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
    todo!()

//     		assertEquals(null, m.getPlainBodyText());
    // 		assertEquals(null, m.getHtmlBodyText());
    //
    // 		// messages are handled as attached files.
    // 		assertEquals(0, m.getAttachedMessages().size());
    // 		assertEquals(1, m.getAttachedFiles().size());
    //
    // 		SmtpAttachment attachedFile = m.getAttachedFiles().get(0);
    // 		SmtpMessage attached = mimeStringToSmtpMessageConverter.mimeToSmtpMessage(mimeStringToSmtpMessageConverter.dataToMimeMessage(attachedFile.getData()),
    // 				null);
    //
    // 		assertEquals("attached message", attached.getSubject());
    // 		assertEquals(null, attached.getHtmlBodyText());
    //
    // 		assertEquals(0, attached.getAttachedMessages().size());
    // 		assertEquals(1, attached.getAttachedFiles().size());
    //
    // 		SmtpAttachment indirectAttachment = attached.getAttachedFiles().get(0);
    // 		assertEquals("indirectly_attached.txt", indirectAttachment.getName());
    // 		assertArrayEquals(EncodingConverter.base64ToBytes("Zmlyc3QgYXR0YWNobWVudA=="), indirectAttachment.getData());
}

#[test]
fn text_attachment() {
    let msg = r#"Subject: text attachment
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: multipart/mixed; boundary=frontier

--frontier
Content-type: text/plain; charset=UTF-8

Tutanota: äüöß€*#\\{³|@
--frontier
Content-type: text/plain; charset=UTF-8
Content-Disposition: attachment; filename=a1.txt;

Abc, die Katze liegt im Schnee ! äöü?ß !
--frontier--
"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
    todo!()

//     		assertEquals("text attachment", m.getSubject());
    // 		assertEquals("Tutanota: äüöß€*#\\{³|@", m.getPlainBodyText());
    // 		assertEquals(null, m.getHtmlBodyText());
    // 		assertEquals(date, m.getSentDate());
    //
    // 		assertEquals(1, m.getAttachedFiles().size());
    // 		SmtpAttachment a1 = m.getAttachedFiles().get(0);
    //
    // 		assertEquals("a1.txt", a1.getName());
    // 		assertArrayEquals("Abc, die Katze liegt im Schnee ! äöü?ß ! ".getBytes("UTF-8"), a1.getData());
}

#[test]
fn html_attachment() {
    let msg = r#"Subject: html attachment
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: multipart/mixed; boundary=frontier

--frontier
Content-type: text/plain; charset=UTF-8

Tutanota: äüöß€*#\\{³|@
--frontier
Content-type: text/html; charset=UTF-8
Content-Disposition: attachment; filename=a1.html;

<html><body><b><small>Hello äöüß</small></b><br></body></html>
--frontier--
"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
    todo!()

//     		assertEquals("html attachment", m.getSubject());
    // 		assertEquals("Tutanota: äüöß€*#\\{³|@", m.getPlainBodyText());
    // 		assertEquals(null, m.getHtmlBodyText());
    // 		assertEquals(date, m.getSentDate());
    //
    // 		assertEquals(1, m.getAttachedFiles().size());
    // 		SmtpAttachment a1 = m.getAttachedFiles().get(0);
    //
    // 		assertEquals("a1.html", a1.getName());
    // 		assertArrayEquals("<html><body><b><small>Hello äöüß</small></b><br></body></html>".getBytes("UTF-8"), a1.getData());
}

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

//     		String firstPlainBodyText = "Tutanota: äüöß€*#\\{³|@";
    // 		String secondPlainBodyText = "Abc, die Katze liegt im Schnee ! äöü?ß !";
    // 		StringBuilder b = new StringBuilder()
    // 				.append("Subject: multiple text/plain parts concatenated\n")
    // 				.append("From: A <a@tutanota.de>\n")
    // 				.append("To: B <b@tutanota.de>\n")
    // 				.append("Date: " + new MailDateFormat().format(date) + "\n")
    // 				.append("Content-Type: multipart/mixed; boundary=frontier\n")
    // 				.append("\n")
    // 				.append("--frontier\n")
    // 				.append("Content-type: text/plain; charset=UTF-8\n")
    // 				.append("\n")
    // 				.append(firstPlainBodyText + "\n")
    // 				.append("--frontier\n")
    // 				.append("Content-type: text/plain; charset=UTF-8\n")
    // 				.append("\n")
    // 				.append(secondPlainBodyText + "\n")
    // 				.append("--frontier--");
    //
    // 		SmtpMessage m = mimeStringToSmtpMessageConverter.mimeToSmtpMessage(
    // 				mimeStringToSmtpMessageConverter.dataToMimeMessage(b.toString().getBytes(StandardCharsets.UTF_8)),
    // 				null
    // 		);
    //
    // 		assertEquals("multiple text/plain parts concatenated", m.getSubject());
    // 		String concatenatedPlainBodyText = firstPlainBodyText.concat(secondPlainBodyText);
    // 		assertEquals(concatenatedPlainBodyText, m.getPlainBodyText());
    // 		assertEquals(null, m.getHtmlBodyText());
    // 		assertEquals(date, m.getSentDate());
    // 		assertEquals(0, m.getAttachedFiles().size());
}

#[test]
fn multiple_html_body_text_parts_are_concatenated() {
    let msg = r#"Message-Id: some-id
Subject: multiple text/html parts concatenated
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: multipart/mixed; boundary=frontier

--frontier
Content-type: text/html; charset=UTF-8

<html><body><b><small>Hello äöüß</small></b><br></body></html>
--frontier
Content-type: text/html; charset=UTF-8

<html><body><b><small>Test Test</small></b><br></body></html>
--frontier--
"#;

    let m = parse_mail(msg);

    assert_eq!("multiple text/html parts concatenated", m.subject);
    assert_eq!("<html><body><b><small>Hello äöüß</small></b><br></body></html><html><body><b><small>Test Test</small></b><br></body></html>", m.html_body_text);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
    assert_eq!(0, m.attachments.len());
}

#[test]
// todo! what does this test (map)
fn concatenate_alternative_html_text_parts() {
    let msg = r#"Message-Id: some-id
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
    let _m = parse_mail(msg);
    todo!()
}

#[test]
// todo! what does this test (map)
fn concatenate_multiple_html_and_plain_text_parts() {
    let msg = r#"Message-Id: some-id
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

    let _m = parse_mail(msg);
    todo!()
}

#[test]
fn plain_body_text_parts_are_concatenated_with_html_body_parts_if_html_body_parts_already_existing()
{
    let msg = r#"Subject: multiple text/html and text/plain parts concatenated to single text/html
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: multipart/mixed; boundary=frontier

--frontier
Content-type: text/html; charset=UTF-8

<html><body><b><small>Hello äöüß</small></b><br></body></html>
--frontier
Content-type: text/html; charset=UTF-8

<html><body><b><small>Test Test</small></b><br></body></html>
--frontier
Content-type: text/plain; charset=UTF-8

Abc, die Katze liegt im Schnee ! äöü?ß !
--frontier-
"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(
        "multiple text/html and text/plain parts concatenated to single text/html",
        m.subject
    );
    todo!()

//     		assertEquals("multiple text/html and text/plain parts concatenated to single text/html", m.getSubject());
    // 		String concatenatedHtmlBodyText = firstHtmlBodyText.concat(secondHtmlBodyText).concat(PlainTextHtmlConverter.plainTextToHtml(firstPlainBodyText));
    // 		assertEquals(null, m.getPlainBodyText());
    // 		assertEquals(concatenatedHtmlBodyText, m.getHtmlBodyText());
    // 		assertEquals(date, m.getSentDate());
    // 		assertEquals(0, m.getAttachedFiles().size());
}

#[test]
fn plain_body_text_parts_are_converted_to_html_body_parts_if_html_body_parts_follow_afterwards() {
    let msg = r#"

"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
	todo!()

//     		assertEquals("multiple plain/text and text/html parts concatenated to single text/html", m.getSubject());
    // 		String concatenatedHtmlBodyText = PlainTextHtmlConverter.plainTextToHtml(firstPlainBodyText).concat(firstHtmlBodyText).concat(secondHtmlBodyText);
    // 		assertEquals(null, m.getPlainBodyText());
    // 		assertEquals(concatenatedHtmlBodyText, m.getHtmlBodyText());
    // 		assertEquals(date, m.getSentDate());
    // 		assertEquals(0, m.getAttachedFiles().size());
}

#[test]
fn text_attachment_with_disposition() {
    let msg = r#"Subject: text attachment
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: " + new MailDateFormat().format(date) + "
Content-type: text/plain; charset=UTF-8
Content-Disposition: attachment; filename=a1.txt;

Abc, die Katze liegt im Schnee ! äöü?ß !
"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
	todo!()

//     		assertEquals("text attachment", m.getSubject());
    // 		assertNull(m.getPlainBodyText());
    // 		assertNull(m.getHtmlBodyText());
    // 		assertEquals(date, m.getSentDate());
    //
    // 		assertEquals(1, m.getAttachedFiles().size());
    // 		SmtpAttachment a1 = m.getAttachedFiles().get(0);
    //
    // 		assertEquals("a1.txt", a1.getName());
    // 		assertArrayEquals("Abc, die Katze liegt im Schnee ! äöü?ß ! ".getBytes("UTF-8"), a1.getData());
}

#[test]
fn attachment_with_non_ascii_name() {
	let msg = r#"Subject: text attachment
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: " + new MailDateFormat().format(date) + "
Content-type: text/plain; charset=UTF-8; name=\"=?ISO-8859-1?Q?a=F6i=2Epdf?=\"
Content-Disposition: attachment; filename*=ISO-8859-1''%61%F6%69%2E%70%64%66

Abc, die Katze liegt im Schnee ! äöü?ß ! "#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
	todo!()

//     		SmtpAttachment a1 = m.getAttachedFiles().get(0);
    // 		assertEquals("aöi.pdf", a1.getName());
}

#[test]
fn attachment_filename_in_content_type() {
	let msg = r#"Subject: message with named file attachment
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: application/octet-stream; name=indirectly_attached.txt;
Content-Transfer-Encoding: base64

Zmlyc3QgYXR0YWNobWVudA=="#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
	todo!()
//     		SmtpAttachment indirectAttachment = m.getAttachedFiles().get(0);
    // 		assertEquals("indirectly_attached.txt", indirectAttachment.getName());
    // 		assertArrayEquals(EncodingConverter.base64ToBytes("Zmlyc3QgYXR0YWNobWVudA=="), indirectAttachment.getData());
}

#[test]
fn attachment_filename_qencoding() {
	let msg = r#"Subject: message with named file attachment
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: " + new MailDateFormat().format(date) + "
Content-type: application/octet-stream; name==?utf-8?Q?=C3=A4=C3=B6=C3=9F=E2=82=AC.txt?=;
Content-Transfer-Encoding: base64

Zmlyc3QgYXR0YWNobWVudA=="#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
	todo!()

// 		SmtpAttachment indirectAttachment = m.getAttachedFiles().get(0);
    // 		assertEquals("äöß€.txt", indirectAttachment.getName());
    // 		assertArrayEquals(EncodingConverter.base64ToBytes("Zmlyc3QgYXR0YWNobWVudA=="), indirectAttachment.getData());
}

#[test]
fn encrypted() {
	let msg = r#"Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: multipart/encrypted; boundary=frontier

--frontier
Content-Type: application/octet-stream
Content-Transfer-Encoding: base64

SGFsbG8=
--frontier--"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
	todo!()

//     		assertEquals(1, m.getAttachedFiles().size());
}

#[test]
fn recipient_groups() {
	let msg = r#"Subject: Hello
From: A <a@tutanota.de>
To: foo:a@b.example.de,c@d.example.de,e@f.example.de;
Reply-To: ??? <???@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    assert_eq!(m.to_addresses, vec![("B", "b@tutanota.de").into()]);
    assert_eq!(Some(DateTime::from_millis(1730991244000)), m.date);
	todo!()

//     		assertEquals("a@b.example.de", m.getToRecipients().get(0).getMailAddress());
    // 		assertEquals("c@d.example.de", m.getToRecipients().get(1).getMailAddress());
    // 		assertEquals("e@f.example.de", m.getToRecipients().get(2).getMailAddress());
}

#[test]
fn undisclosed_recipients() {
	let msg = r#"To: undisclosed-recipients:;"#;
	let m: ImportableMail = parse_mail(msg);

	assert!(m.to_addresses.is_empty());
}

#[test]
fn long_content_type() {
	let msg = r#"From: A <a@tutanota.de>
Content-type: multipart/mixed; boundary=frontier

--frontier
Content-Type: text/plain; charset=us-ascii; name=withoutContentType.pdf
Content-Disposition: attachment; filename=withoutContentType.pdf;

Message
--frontier--
"#;
    let _m: ImportableMail = parse_mail(msg);

	todo!()
//     		assertEquals("text/plain", m.getAttachedFiles().get(0).getMimeType());
    // 		assertEquals("us-ascii", m.getAttachedFiles().get(0).getCharset());
    // 		assertEquals("withoutContentType.pdf", m.getAttachedFiles().get(0).getName());
}

#[test]
fn normalize_header_value() {
    todo!()
// 		// trim and remove LF and CR
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds(" <a@b>").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds("<a@b> ").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds(" \r <a@b> \n ").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds(" \n <a@b> \r ").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds("\n\r <a@b> \r\n").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds(" | <a@b>\n\r | ").toArray(new String[0]));
    // 		assertArrayEquals(new String[0], MimeStringToSmtpMessageConverter.stripMessageIds(" <>").toArray(new String[0]));
    //
    // 		// remove comments
    // 		assertArrayEquals(new String[0], MimeStringToSmtpMessageConverter.stripMessageIds(" (abc)").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds(" (abc) <a@b>").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds(" (abc) <a@b> (def) ").toArray(new String[0]));
    //
    // 		// illegal comments
    // 		assertArrayEquals(new String[0], MimeStringToSmtpMessageConverter.stripMessageIds(" (abc").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds(" abc) <a@b>").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds(" abc <a@b> (abc").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds(" )abc <a@b> def( ").toArray(new String[0]));
    //
    // 		// ids in comments are currently recognized
    // 		assertArrayEquals(new String[]{"a@b", "g@h", "i@d"},
    // 				MimeStringToSmtpMessageConverter.stripMessageIds(" (abc) <a@b> (def) <g@h> (<i@d>)").toArray(new String[0]));
    // 		assertArrayEquals(new String[]{"a@b"}, MimeStringToSmtpMessageConverter.stripMessageIds(" (abc <a@b> def) ").toArray(new String[0]));
}

#[test]
fn get_spf_result() {
    // net yet used on rust
}

#[test]
fn mail_from_with_delemiter() {
    let msg = r#"
Message-ID: 123456
Subject: Hello
From: A,B <a@external.de>
To: B <b@tutanota.de>
References: <sadf@tutanota.de> <1234564@web.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: multipart/mixed; boundary=frontier

--frontier
"#;
    let _m: ImportableMail = parse_mail(msg);

	todo!()
//     		assertEquals("A, B <a@external.de>", m.getSender().getMailAddress());
    // 		assertEquals("", m.getSender().getName());
    // 		assertFalse(m.getSender().isValid());
}

#[test]
fn incomplete_text_content_type() {
    let msg = r#"
Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-type: text

any body text


--frontier
"#;
    let _m: ImportableMail = parse_mail(msg);

	todo!()
//     		assertEquals("any body text", m.getPlainBodyText());
}

#[test]
fn calendar_content_type() {
	let msg = r#"Message-ID: 123456
Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
References: <sadf@tutanota.de> <1234564@web.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: text/calendar; charset=\"UTF-8\"; method=REQUEST
"#;
    let _m: ImportableMail = parse_mail(msg);

	todo!()
//     		assertEquals("a@tutanota.de", m.getSender().getMailAddress());
    // 		assertEquals("A", m.getSender().getName());
    // 		assertEquals("text/calendar", m.getAttachedFiles().get(0).getMimeType());
    // 		assertEquals("REQUEST", m.getAttachedFiles().get(0).getCalendarMethod());
}

#[test]
fn calendar_content_type_method() {
    let msg = r#"Message-ID: 123456
Subject: Hello
From: A <a@tutanota.de>
To: B <b@tutanota.de>
References: <sadf@tutanota.de> <1234564@web.de>
Date: Thu, 7 Nov 2024 15:54:04 +0100
Content-Type: text/calendar; charset="UTF-8"; method=request;
"#;
    let m: ImportableMail = parse_mail(msg);

    assert_eq!(m.from_addresses, vec![("A", "a@tutanota.de").into()]);
    match m.attachments.first().unwrap() {
        ImportableMailAttachment::Attachment { content_type, .. } => {
            assert_eq!("text/calendar", content_type);
            // todo! assert_eq!("REQUEST", calendar_method)
        }
        ImportableMailAttachment::AttachedMessage { .. } => { panic!("") }
    };
}

#[test]
fn invalid_content_types_default_to_text_plain() {
    let invalid_content_types = vec![
        "Content-Type:",
        "Content-Type: _",
        "Content-Type: text",
        "Content-Type; text/html",
        "Content-Type; invalid/type",
        "Content-Type: application/pdf; no_parameter_name.pdf"
    ];
    for invalid_content_type in invalid_content_types {
        let parsed = MessageParser::default()
            .parse(invalid_content_type)
            .unwrap();
        assert_eq!("text/plain", parsed.content_type().unwrap().c_type.to_string());
    }
}

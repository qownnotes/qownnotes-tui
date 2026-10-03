use std::time::Duration;

use scraper::{Html, Selector};

/// Only fetch a single, absolute HTTP(S) URL from the clipboard.
pub fn clipboard_url(text: &str) -> Option<&str> {
    let text = text.trim();
    let uri: ureq::http::Uri = text.parse().ok()?;
    if !matches!(uri.scheme_str(), Some("http" | "https"))
        || uri.host().is_none_or(str::is_empty)
        || text.chars().any(char::is_whitespace)
    {
        return None;
    }
    Some(text)
}

pub fn fetch_title(url: &str) -> anyhow::Result<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .max_redirects(5)
        .build()
        .into();
    let html = agent
        .get(url)
        .header(
            "User-Agent",
            concat!("qownnotes-tui/", env!("CARGO_PKG_VERSION")),
        )
        .call()?
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_to_string()?;
    page_title(&html).ok_or_else(|| anyhow::anyhow!("page has no title"))
}

fn page_title(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("title").expect("valid title selector");
    let title = document
        .select(&selector)
        .next()?
        .text()
        .collect::<String>();
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    (!title.is_empty()).then_some(title)
}

pub fn markdown_link(title: &str, url: &str) -> String {
    let mut escaped_title = String::new();
    for character in title.chars() {
        if matches!(character, '\\' | '[' | ']' | '*' | '_' | '`' | '<' | '>') {
            escaped_title.push('\\');
        }
        escaped_title.push(character);
    }
    let destination = url
        .replace('\\', "%5C")
        .replace('(', "%28")
        .replace(')', "%29")
        .replace('<', "%3C")
        .replace('>', "%3E");
    format!("[{escaped_title}]({destination})")
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::{clipboard_url, fetch_title, markdown_link, page_title};

    #[test]
    fn accepts_only_single_absolute_web_urls() {
        assert_eq!(
            clipboard_url(" https://example.org/a?q=1#top\n"),
            Some("https://example.org/a?q=1#top")
        );
        assert_eq!(
            clipboard_url("http://localhost:8080/"),
            Some("http://localhost:8080/")
        );
        for text in [
            "text",
            "example.org",
            "file:///tmp/note",
            "https://",
            "https://example.org/ extra",
            "https://one.org\nhttps://two.org",
        ] {
            assert_eq!(clipboard_url(text), None, "{text}");
        }
    }

    #[test]
    fn decodes_html_titles_and_escapes_markdown() {
        let title = page_title("<html><head><TITLE> Café &amp; &#x5B;notes&#93;\n  &lt;guide&gt; </TITLE></head></html>").unwrap();
        assert_eq!(title, "Café & [notes] <guide>");
        assert_eq!(
            markdown_link(&title, "https://example.org/a(b)?q=1&x=2"),
            "[Café & \\[notes\\] \\<guide\\>](https://example.org/a%28b%29?q=1&x=2)"
        );
        assert_eq!(page_title("<title> \n </title>"), None);
        assert_eq!(page_title("<p>No title</p>"), None);
    }

    #[test]
    fn fetches_titles_follows_redirects_and_reports_http_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for response in [
                "HTTP/1.1 302 Found\r\nLocation: /page\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
                {
                    let body = "<title>Fetched &amp; decoded</title>";
                    format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
                },
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        assert_eq!(
            fetch_title(&format!("{url}/redirect")).unwrap(),
            "Fetched & decoded"
        );
        assert!(fetch_title(&format!("{url}/missing")).is_err());
        server.join().unwrap();
    }
}

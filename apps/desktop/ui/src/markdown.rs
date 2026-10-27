use beholder_core::ImageHash;
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};

const OPTIONS: Options = Options::ENABLE_TABLES
    .union(Options::ENABLE_STRIKETHROUGH)
    .union(Options::ENABLE_TASKLISTS);

pub fn render(src: &str, image_base: &str) -> String {
    let mut dropped_image = false;
    let events = Parser::new_ext(src, OPTIONS).map(|event| match event {
        Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => match image_src(&dest_url, image_base) {
            Some(src) => Event::Start(Tag::Image {
                link_type,
                dest_url: src.into(),
                title,
                id,
            }),
            None => {
                dropped_image = true;
                Event::Start(Tag::Emphasis)
            }
        },
        Event::End(TagEnd::Image) if dropped_image => {
            dropped_image = false;
            Event::End(TagEnd::Emphasis)
        }
        Event::Start(Tag::Link { dest_url, .. }) => Event::InlineHtml(CowStr::from(format!(
            "<span class=\"link\" title=\"{}\">",
            escape_attr(&dest_url)
        ))),
        Event::End(TagEnd::Link) => Event::InlineHtml(CowStr::Borrowed("</span>")),
        other => other,
    });
    let mut out = String::with_capacity(src.len() * 3 / 2);
    html::push_html(&mut out, events);
    out
}

fn image_src(dest: &str, image_base: &str) -> Option<String> {
    let hash = ImageHash::from_hex(dest.strip_prefix("img:")?)?;
    Some(format!("{image_base}display/{}", hash.to_hex()))
}

fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

pub fn excerpt(src: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut count = 0;
    for event in Parser::new_ext(src, OPTIONS) {
        let piece = match &event {
            Event::Text(t) | Event::Code(t) => t.as_ref(),
            Event::SoftBreak
            | Event::HardBreak
            | Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item) => " ",
            _ => continue,
        };
        for c in piece.chars() {
            if count == max_chars {
                return format!("{}…", out.trim_end());
            }
            if c == ' ' && out.ends_with(' ') {
                continue;
            }
            out.push(c);
            count += 1;
        }
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "img://localhost/";

    #[test]
    fn raw_html_is_escaped() {
        let out = render(
            "<script>alert(1)</script>\n\nhi <img src=x onerror=alert(1)>",
            BASE,
        );
        assert!(!out.contains("<script"), "{out}");
        assert!(!out.contains("<img"), "{out}");
        assert!(out.contains("&lt;script&gt;"), "{out}");
    }

    #[test]
    fn only_server_images_load() {
        let hash = ImageHash([0xab; 32]);
        let ok = render(&format!("![seal](img:{})", hash.to_hex()), BASE);
        assert!(
            ok.contains(&format!(
                "src=\"img://localhost/display/{}\"",
                hash.to_hex()
            )),
            "{ok}"
        );
        for bad in [
            "![x](https://evil.example/p.png)",
            "![x](img:nothex)",
            "![x](javascript:alert(1))",
        ] {
            let out = render(bad, BASE);
            assert!(!out.contains("<img"), "{bad} -> {out}");
            assert!(out.contains("<em>x</em>"), "{bad} -> {out}");
        }
    }

    #[test]
    fn links_are_inert() {
        let out = render("[map](https://x.example/?a=1&b=2)", BASE);
        assert!(!out.contains("<a"), "{out}");
        assert!(
            out.contains("<span class=\"link\" title=\"https://x.example/?a=1&amp;b=2\">"),
            "{out}"
        );
        assert!(out.contains(">map</span>"), "{out}");
        let quoted = render("[q](<a\"b>)", BASE);
        assert!(quoted.contains("title=\"a&quot;b\""), "{quoted}");
    }

    #[test]
    fn formatting_survives() {
        let out = render("# Title\n\n**bold** ~~gone~~\n\n| a |\n|---|\n| b |", BASE);
        for tag in ["<h1>", "<strong>", "<del>", "<table>"] {
            assert!(out.contains(tag), "{tag} in {out}");
        }
    }

    #[test]
    fn excerpts_are_plain_and_short() {
        assert_eq!(
            excerpt("# Hi\n\nSome *text*\nhere.", 100),
            "Hi Some text here."
        );
        assert_eq!(excerpt("abcdef", 3), "abc…");
        assert_eq!(excerpt("", 10), "");
    }
}

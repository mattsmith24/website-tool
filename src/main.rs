use serde::{Deserialize, Serialize};
use toml;

#[derive(Debug, Deserialize, Serialize)]
pub struct Page {
    pub title: String,
    pub menu: String,
    pub path: String,
    pub basename: String,
    pub template: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Sitemap {
    pub pages: Vec<Page>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub markdown_content: String,
    pub static_files: String,
    pub templates: String,
    pub serve: String,
    pub sitemap: String,
}

pub fn parse_config(path: &str) -> Result<Config, toml::de::Error> {
    let content = std::fs::read_to_string(path).unwrap();
    toml::from_str(&content)
}

pub fn parse_sitemap(path: &str) -> Result<Sitemap, serde_json::Error> {
    let content = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&content)
}

fn build_context(sitemap: &Sitemap, content: &str, page: &Page) -> tera::Context {
    let mut ctx = tera::Context::new();
    ctx.insert("title", &page.title);
    ctx.insert("content", content);
    ctx.insert("pages", &sitemap.pages);
    ctx.insert("selected", format!("{}{}", &page.path, &page.basename).as_str());
    ctx
}

fn build_path(basepath: &str, suffix: &str, page: &Page) -> String {
    if page.path == "/" {
        format!("{}/{}.{}", basepath, page.basename, suffix)
    } else {
        format!("{}/{}{}.{}", basepath, page.path, page.basename, suffix)
    }
}

fn render_markdown_file(path: &str) -> String {
    let content = std::fs::read_to_string(path).expect("Failed to read file");
    markdown::to_html(&content)
}


fn copy_static_to_serve(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_static_to_serve(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn main() {
    let config = parse_config("config.toml").expect("Failed to parse config");
    let mut tera = tera::Tera::new(&format!("{}/**/*", config.templates)).expect("Failed to parse templates");
    tera.autoescape_on(vec![]);
    let sitemap = parse_sitemap(&config.sitemap).expect("Failed to parse sitemap");
    for page in &sitemap.pages {
        let file_path = build_path(&config.markdown_content, "md", page);
        let html_content = render_markdown_file(&file_path);
        let ctx = build_context(&sitemap, &html_content, &page);
        let rendered = tera.render(&page.template, &ctx).unwrap();
        let out_path = build_path(&config.serve, "html", page);
        std::fs::write(&out_path, &rendered).expect("Failed to write file");
    }
    copy_static_to_serve(
        std::path::Path::new(&config.static_files),
        std::path::Path::new(&config.serve),
    )
    .expect("Failed to copy static files");
}
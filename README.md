# Website Tool

This is a basic website generator that takes some markdown files and templates
and produces HTML files for a website.

The goals are to avoid creating a complex node application with many NPMs or a
python / php / ruby application that requires complex runtime caching and a
database.

## Configuration

Edit `config.toml` like this:

```toml
markdown_content = "path/to/markdown/content"
static_files = "path/to/static/files"
templates = "path/to/templates"
serve = "path/to/serve"
sitemap = "path/to/sitemap.json"
```

## Creating content

Write content for pages as markdown in `markdown_content`.

Create a `sitemap.json` like this:

```json
{
    "pages": [
        {
            "title": "Main",
            "menu": "Main",
            "path": "/",
            "basename": "index",
            "template": "main.html"
        },
        {
            "title": "This is the \"About\"' page, isn't it cool?",
            "menu": "About",
            "path": "/",
            "basename": "about",
            "template": "main.html"
        }
    ]
}
```

The fields have these meanings:

**title** - Text set in the `<title>` element for the page.

**menu** - Text used when generating the menu (can be shorter than the title)

**path** - Path inside the content directory e.g. if you have sub-pages in the
'blog' directory.

**basename** - Name of the markdown file without the `.md` suffix. e.g. if the
file is named `welcome.md`, put "welcome" in this field. It will be served as
`welcome.html`.

**template** - Name of the template file under `templates`.


## Usage

```shell
cargo run
```

## How It Works

The markdown in `markdown_content` is rendered as HTML.

The Tera `templates` are filled using the site-map config to render the navigation
and pages (filling in content from the rendered markdown). These are output to
the `serve` directory.

The `static_files` are copied to the `serve` directory.

The web server is configured to serve content from the `serve` directory.

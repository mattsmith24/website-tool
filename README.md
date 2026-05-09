# Website Tool

This is a basic website generator that takes some markdown files and templates
and produces HTML files for a website.

The goals are to avoid creating a complex node application with many NPMs or a
python / php / ruby application that requires complex runtime caching and a
database.

## Configuration

Create a `config.toml` like this:

```toml
markdown_content = "path/to/markdown/content"
static_files = "path/to/static/files"
templates = "path/to/templates"
serve = "path/to/serve"
sitemap = "path/to/sitemap.json"
```

## Creating content

### Page Contents

Write content for pages as markdown in `markdown_content`.

For more info about markdown, read the [markdown
website](https://www.markdownguide.org/)

### Static Files

Files like images, css etc should be saved in `static_files`.

### Templates

Create a template in `templates` using the [Tera
format](https://keats.github.io/tera/).

The variables made available to the template are set in the sitemap (see below).

Variables: `title`, `path` and `basename` for the current page are set in the
context as well as the full `pages` structure for building navigation.

A variable `selected` is set to the current page with path for use in
highlighting the current page in the menu.

### Sitemap

Create a `sitemap.json` like this:

```json
{
    "pages": [
        {
            "title": "Home",
            "menu": "Home",
            "path": "/",
            "basename": "index",
            "template": "main.html",
            "subpages": []
        },
        {
            "title": "Blog",
            "menu": "Blog",
            "path": "/",
            "basename": "blog",
            "template": "blog.html",
            "subpages": [
                {
                    "title": "Something something blah",
                    "menu": "Something",
                    "path": "/blog/2026/",
                    "basename": "0509-something",
                    "template": "blogpost.html",
                    "subpages": []
                }
            ]
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

The paths and basenames can be whatever you want. The blog example is just my
system.

## Usage

```shell
cargo run -- --config path/to/config.toml
```

## How It Works

The markdown in `markdown_content` is rendered as HTML.

The Tera `templates` are filled using the site-map config to render the navigation
and pages (filling in content from the rendered markdown). These are output to
the `serve` directory.

The `static_files` are copied to the `serve` directory.

The web server is configured to serve content from the `serve` directory.

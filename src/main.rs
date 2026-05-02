use askama::Template;
use axum::{Router, response::Html, routing::get};
use tower_http::services::ServeDir;

// ---------- Data structures ----------

#[derive(Clone)]
struct StoryHeader {
    title: String,
    snippet: String,
}

#[derive(Clone)]
struct TocEntry {
    anchor: String,
    label: String,
}

#[derive(Clone)]
struct Section {
    id: String,
    heading: String,
    paragraphs: Vec<String>,
}

// ---------- Template ----------

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    site_title: String,
    page_title: String,
    theme: String,
    stories: Vec<StoryHeader>,
    toc: Vec<TocEntry>,
    sections: Vec<Section>,
    year: u16,
}

// ---------- Handlers ----------

async fn index() -> Html<String> {
    let sections = vec![
        Section {
            id: "introduction".into(),
            heading: "Introduction".into(),
            paragraphs: vec![
                "Welcome to a small demonstration of server-side rendered HTML, \
                 written entirely in Rust. The page you're reading was assembled \
                 on the server before being delivered to your browser."
                    .into(),
                "The layout uses three columns: stories on the left, the article \
                 in the middle, and a table of contents on the right."
                    .into(),
                "The layout uses three columns: stories on the left, the article \
                 in the middle, and a table of contents on the right."
                    .into(),
                "The layout uses three columns: stories on the left, the article \
                 in the middle, and a table of contents on the right."
                    .into(),
                "The layout uses three columns: stories on the left, the article \
                 in the middle, and a table of contents on the right."
                    .into(),
            ],
        },
        Section {
            id: "why-rust".into(),
            heading: "Why Rust on the server?".into(),
            paragraphs: vec![
                "Rust gives you strong guarantees at compile time, predictable \
                 performance, and a small runtime footprint. For server-rendered \
                 pages, that translates into fast responses and confident deployments."
                    .into(),
                "Frameworks like Axum keep the request handling lightweight, while \
                 Askama compiles your templates into native Rust code."
                    .into(),
                "The layout uses three columns: stories on the left, the article \
                 in the middle, and a table of contents on the right."
                    .into(),
                "The layout uses three columns: stories on the left, the article \
                 in the middle, and a table of contents on the right."
                    .into(),
                "The layout uses three columns: stories on the left, the article \
                 in the middle, and a table of contents on the right."
                    .into(),
            ],
        },
        Section {
            id: "theming".into(),
            heading: "Theming".into(),
            paragraphs: vec![
                "All visual styling lives in separate CSS files under /static/css. \
                 The base layout sits in base.css, and each theme defines its own \
                 palette using CSS custom properties."
                    .into(),
                "Switching themes is as easy as changing the linked stylesheet — \
                 try /?theme=dark or /?theme=sepia in a future iteration."
                    .into(),
            ],
        },
        Section {
            id: "closing".into(),
            heading: "Closing thoughts".into(),
            paragraphs: vec![
                "This is a starting point. From here you can plug in a database, \
                 add markdown rendering, or wire up a richer routing layer."
                    .into(),
            ],
        },
    ];

    let toc = sections
        .iter()
        .map(|s| TocEntry {
            anchor: s.id.clone(),
            label: s.heading.clone(),
        })
        .collect();

    let stories = vec![
        StoryHeader {
            title: "The quiet rise of Rust on the web".into(),
            snippet: "How a systems language found its way into HTTP servers.".into(),
        },
        StoryHeader {
            title: "Server-side rendering, again".into(),
            snippet: "The pendulum swings back toward HTML over the wire.".into(),
        },
        StoryHeader {
            title: "Small binaries, big confidence".into(),
            snippet: "Why teams are deploying single-binary web apps.".into(),
        },
        StoryHeader {
            title: "Templating in Rust".into(),
            snippet: "Askama, Maud, and Tera compared.".into(),
        },
    ];

    let tmpl = IndexTemplate {
        site_title: "Rusty Pages".into(),
        page_title: "A server-rendered page in Rust".into(),
        theme: "light".into(),
        stories,
        toc,
        sections,
        year: 2026,
    };

    Html(tmpl.render().unwrap())
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index))
        .nest_service("/static", ServeDir::new("static"));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("Listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}

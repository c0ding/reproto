extern crate lalrpop_snap;
extern crate toml;
extern crate handlebars;
extern crate serde_json;

use std::fs::File;
use std::io::{Write, Read};
use serde_json::value::Map;

fn read_file(path: &str) -> String {
    let mut f = File::open(path).map_err(|e| format!("cannot open: {}: {}", path, e)).unwrap();
    let mut content = String::new();
    f.read_to_string(&mut content).unwrap();
    content
}

fn process_colors() {
    println!("hello");

    let colors_content = read_file("colors.toml");
    let template_content = read_file("src/backend/doc/static/doc._.css.hbs");

    let value: toml::Value = colors_content.parse().unwrap();
    let schemes = value.as_table().unwrap();

    let mut handlebar = handlebars::Handlebars::new();

    handlebar.register_template_string("doc", template_content).unwrap();

    for (key, value) in schemes {
        let colors_in = value.as_table().unwrap();
        let mut colors = Map::new();

        for (k, color) in colors_in {
            let value = color.as_str().unwrap();
            colors.insert(k.to_owned(), handlebars::to_json(&value));
        }

        let result = handlebar.render("doc", &colors).unwrap();

        let mut f = File::create(&format!("src/backend/doc/static/doc.{}.css", key)).unwrap();
        f.write_all(&result.into_bytes()).unwrap();
    }
}

fn main() {
    process_colors();
    lalrpop_snap::process_root().unwrap();
}

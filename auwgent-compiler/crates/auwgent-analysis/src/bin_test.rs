use auwgent_analysis::hover::hover_for_source;
use std::path::Path;
use std::fs;


fn main() {
    let source = fs::read_to_string("c:/Users/babyface/Desktop/auwgent/Auwgent/test.agent").unwrap();
    let path = Path::new("c:/Users/babyface/Desktop/auwgent/Auwgent/test.agent");

    // Find "a" in "let a:"
    if let Some(offset) = source.find("a: A") {
        println!("Hovering at offset {}", offset);
        let hover = hover_for_source(&path, &source, offset);
        println!("Hover for 'a': {:?}", hover);
    }

    // Find "oops" in "let oops ="
    if let Some(offset) = source.find("oops =") {
        println!("Hovering at offset {}", offset);
        let hover = hover_for_source(&path, &source, offset);
        println!("Hover for 'oops': {:?}", hover);
    }

    // Find "one" in "one("123")"
    if let Some(offset) = source.find("one(\"123\")") {
        println!("Hovering at offset {}", offset);
        let hover = hover_for_source(&path, &source, offset);
        println!("Hover for 'one': {:?}", hover);
    }
}

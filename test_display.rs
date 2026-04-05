use core_graphics::display::CGDisplay;
fn main() {
    let display = CGDisplay::main();
    let physical_width = display.pixels_wide() as f64;
    let logical_width = display.bounds().size.width as f64;
    println!("Physical: {}, Logical: {}", physical_width, logical_width);
}

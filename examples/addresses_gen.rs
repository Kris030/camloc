use camloc::scanning::{AddressTemplate, TemplateMember::*};

fn main() {
	let template = [Fixed(192), Fixed(168), Fixed(0), Templated(1..18)];
	for s in AddressTemplate::new(template, Fixed(9111)).into_iter() {
		println!("{s}");
	}
}
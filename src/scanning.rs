use std::ops::Range;

#[derive(Debug, Clone)]
pub enum TemplateMember {
    Templated(Range<u8>),
    Fixed(u8),
}

pub struct AddressTemplate {
    template: [TemplateMember; 4],
}
// TODO: proc macro to create from string
impl AddressTemplate {
    pub fn new(template: [TemplateMember; 4]) -> Self { Self { template } }
}

impl IntoIterator for &AddressTemplate {
    type Item = String;
    type IntoIter = std::vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        let mut res = vec![];

		let iter0: Vec<u8> = match &self.template[0] {
			TemplateMember::Fixed(f) => vec![*f],
			TemplateMember::Templated(r) => r.clone().collect(),
		};
		let iter1: Vec<u8> = match &self.template[1] {
			TemplateMember::Fixed(f) => vec![*f],
			TemplateMember::Templated(r) => r.clone().collect(),
		};
		let iter2: Vec<u8> = match &self.template[2] {
			TemplateMember::Fixed(f) => vec![*f],
			TemplateMember::Templated(r) => r.clone().collect(),
		};
		let iter3: Vec<u8> = match &self.template[3] {
			TemplateMember::Fixed(f) => vec![*f],
			TemplateMember::Templated(r) => r.clone().collect(),
		};

		for b1 in &iter0 {
			for b2 in &iter1 {
				for b3 in &iter2 {
					for b4 in &iter3 {
						res.push(format!("{}.{}.{}.{}", b1, b2, b3, b4));
					}
				}
			}
		}

        res.into_iter()
    }
}

use std::ops::Range;

#[derive(Debug, Clone)]
pub enum TemplateMember<T> {
    Templated(Range<T>),
    Fixed(T),
}

#[derive(Debug, Clone)]
pub struct AddressTemplate {
    template: [TemplateMember<u8>; 4],
    port: TemplateMember<u16>,
}
// TODO: proc macro to create from string
impl AddressTemplate {
    pub fn new(template: [TemplateMember<u8>; 4], port: TemplateMember<u16>) -> Self {
		Self { template, port }
	}
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
		let ports: Vec<u16> = match &self.port {
			TemplateMember::Fixed(f) => vec![*f],
			TemplateMember::Templated(r) => r.clone().collect(),
		};

		for b1 in &iter0 {
			for b2 in &iter1 {
				for b3 in &iter2 {
					for b4 in &iter3 {
						for p in &ports {
							res.push(format!("{b1}.{b2}.{b3}.{b4}:{p}"));
						}
					}
				}
			}
		}

        res.into_iter()
    }
}

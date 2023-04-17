use std::ops::RangeInclusive;

#[derive(Debug, Clone)]
pub enum TemplateMember<T> {
    Templated(RangeInclusive<T>),
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

pub struct AddressTemplateIter {
	ranges: [(usize, usize); 5],
	state: [usize; 5],
	last: bool,
}
impl AddressTemplateIter {
    fn new(templ: &AddressTemplate) -> AddressTemplateIter {
        let iter0 = match &templ.template[0] {
			TemplateMember::Fixed(f) => { let f = *f as usize; (f, f)},
			TemplateMember::Templated(r) => (*r.start() as usize, *r.end() as usize),
		};
		let iter1 = match &templ.template[1] {
			TemplateMember::Fixed(f) => { let f = *f as usize; (f, f)},
			TemplateMember::Templated(r) => (*r.start() as usize, *r.end() as usize),
		};
		let iter2 = match &templ.template[2] {
			TemplateMember::Fixed(f) => { let f = *f as usize; (f, f)},
			TemplateMember::Templated(r) => (*r.start() as usize, *r.end() as usize),
		};
		let iter3 = match &templ.template[3] {
			TemplateMember::Fixed(f) => { let f = *f as usize; (f, f)},
			TemplateMember::Templated(r) => (*r.start() as usize, *r.end() as usize),
		};
		let ports = match &templ.port {
			TemplateMember::Fixed(f) => { let f = *f as usize; (f, f)},
			TemplateMember::Templated(r) => (*r.start() as usize, *r.end() as usize),
		};
		Self {
			state: [iter0.0, iter1.0, iter2.0, iter3.0, ports.0],
			ranges: [iter0, iter1, iter2, iter3, ports],
			last: false,
		}
    }
}

impl Iterator for AddressTemplateIter {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
		if self.last {
			return None;
		}
		let ret = format!("{}.{}.{}.{}:{}", self.state[0], self.state[1], self.state[1], self.state[3], self.state[4]);

		let mut i = 4;
		loop {
			if self.state[i] == self.ranges[i].1 {
				self.state[i] = self.ranges[i].0;

				if i == 0 {
					self.last = true;
					return Some(ret);
				} else {
					i -= 1;
				}
			} else {
				self.state[i] += 1;
				break;
			}
		}

		Some(ret)
    }
}

impl IntoIterator for &AddressTemplate {
    type IntoIter = AddressTemplateIter;
    type Item = String;

    fn into_iter(self) -> Self::IntoIter {
		AddressTemplateIter::new(self)
    }
}

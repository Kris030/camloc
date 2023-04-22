use std::ops::RangeInclusive;

#[derive(Debug, Clone)]
pub enum TemplateMember<T> {
    Templated(RangeInclusive<T>),
    Fixed(T),
}

impl<T: Clone, U: From<T>> From<TemplateMember<T>> for RangeInclusive<U> {
    fn from(value: TemplateMember<T>) -> Self {
		match value {
			TemplateMember::Templated(r) =>
				(r.start().clone()).into()..=(r.end().clone()).into(),
	
			TemplateMember::Fixed(v) =>
				{ v.clone().into()..=v.into() },
		}
    }
}

#[derive(Debug, Clone)]
pub struct IPV4AddressTemplate {
    template: [RangeInclusive<usize>; 5],
}
// TODO: proc macro to create from string
impl IPV4AddressTemplate {
    pub fn new(template: [TemplateMember<u8>; 4], port: TemplateMember<u16>) -> Self {
		Self {
			template: [
				template[0].clone().into(),
				template[1].clone().into(),
				template[2].clone().into(),
				template[3].clone().into(),
				port.into(),
			]
		}
	}
}

pub struct IPV4AddressTemplateIter {
	ranges: [RangeInclusive<usize>; 5],
	state: [usize; 5],
	last: bool,
}
impl IPV4AddressTemplateIter {
    fn new(templ: &IPV4AddressTemplate) -> IPV4AddressTemplateIter {
		Self {
			state: templ.template.clone().map(|r| *r.start()),
			ranges: templ.template.clone(),
			last: false,
		}
    }
}

impl Iterator for IPV4AddressTemplateIter {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
		if self.last {
			return None;
		}
		let ret = format!("{}.{}.{}.{}:{}", self.state[0], self.state[1], self.state[1], self.state[3], self.state[4]);

		let mut i = 4;
		loop {
			if self.state[i] == *self.ranges[i].end() {
				self.state[i] = *self.ranges[i].start();

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

impl IntoIterator for &IPV4AddressTemplate {
    type IntoIter = IPV4AddressTemplateIter;
    type Item = String;

    fn into_iter(self) -> Self::IntoIter {
		IPV4AddressTemplateIter::new(self)
    }
}

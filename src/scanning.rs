use std::{net::Ipv4Addr, ops::RangeInclusive};

#[derive(Debug, Clone)]
pub enum TemplateMember<T> {
    Templated(RangeInclusive<T>),
    Fixed(T),
}

impl<T: std::fmt::Display> std::fmt::Display for TemplateMember<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateMember::Templated(r) => write!(f, "[{}, {}]", r.start(), r.end()),
            TemplateMember::Fixed(v) => write!(f, "{v}"),
        }
    }
}

impl<T: Clone + PartialEq, U: From<T>> From<&RangeInclusive<T>> for TemplateMember<U> {
    fn from(value: &RangeInclusive<T>) -> Self {
        if value.start() == value.end() {
            TemplateMember::Fixed(value.start().clone().into())
        } else {
            TemplateMember::Templated(value.start().clone().into()..=value.end().clone().into())
        }
    }
}

impl<T: Clone, U: From<T>> From<TemplateMember<T>> for RangeInclusive<U> {
    fn from(value: TemplateMember<T>) -> Self {
        match value {
            TemplateMember::Templated(r) => (r.start().clone()).into()..=(r.end().clone()).into(),

            TemplateMember::Fixed(v) => v.clone().into()..=v.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IPV4AddressTemplate {
    template: [RangeInclusive<usize>; 5],
}

impl IPV4AddressTemplate {
    pub fn new(template: [TemplateMember<u8>; 4], port: TemplateMember<u16>) -> Self {
        Self {
            template: [
                template[0].clone().into(),
                template[1].clone().into(),
                template[2].clone().into(),
                template[3].clone().into(),
                port.into(),
            ],
        }
    }

    pub fn from_netmask(
        network_prefix: Ipv4Addr,
        subnet_mask_bit_length: usize,
        port: TemplateMember<u16>,
    ) -> Self {
        debug_assert!(
            subnet_mask_bit_length <= 32,
            "subnet_mask_bit_length has to be <= 32"
        );

        let non_masked = subnet_mask_bit_length / 8;
        let fully_masked = (32 - subnet_mask_bit_length) / 8;
        let part_masked = 32 - subnet_mask_bit_length - fully_masked * 8;

        let octets = network_prefix.octets();
        let mut v = vec![];
        for o in &octets[0..non_masked] {
            v.push(TemplateMember::Fixed(*o));
        }

        if part_masked != 0 {
            let mask = !(0xffusize << part_masked) as u8;
            let s = octets[non_masked].min(mask);
            let e = s | mask;
            v.push(TemplateMember::Templated(s..=e));
        }

        for _ in 0..fully_masked {
            v.push(TemplateMember::Templated(0..=255));
        }

        Self::new(v.try_into().unwrap(), port)
    }
}

impl std::fmt::Display for IPV4AddressTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}:{}",
            Into::<TemplateMember<usize>>::into(&self.template[0]),
            Into::<TemplateMember<usize>>::into(&self.template[1]),
            Into::<TemplateMember<usize>>::into(&self.template[2]),
            Into::<TemplateMember<usize>>::into(&self.template[3]),
            Into::<TemplateMember<usize>>::into(&self.template[4]),
        )
    }
}

#[derive(Debug)]
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
        let ret = format!(
            "{}.{}.{}.{}:{}",
            self.state[0], self.state[1], self.state[2], self.state[3], self.state[4]
        );

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

pub fn get_netmask_bits(netmask: Ipv4Addr) -> u32 {
    let octets = netmask.octets();
    let mask_u32 = u32::from_ne_bytes(octets);
    mask_u32.leading_ones()
}

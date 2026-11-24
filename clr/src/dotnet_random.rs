//! Seeded System.Random sequence used by the original Mono runtime.

const MBIG: i32 = i32::MAX;
const MSEED: i32 = 161803398;

pub struct DotNetRandom {
    seed_array: [i32; 56],
    inext: usize,
    inextp: usize,
}

impl DotNetRandom {
    pub fn new(seed: i32) -> Self {
        let subtraction = if seed == i32::MIN { i32::MAX } else { seed.abs() };
        let mut seed_array = [0; 56];
        let mut mj = MSEED - subtraction;
        seed_array[55] = mj;
        let mut mk = 1;
        for i in 1..55 {
            let ii = (21 * i) % 55;
            seed_array[ii] = mk;
            mk = mj.wrapping_sub(mk);
            if mk < 0 { mk = mk.wrapping_add(MBIG); }
            mj = seed_array[ii];
        }
        for _ in 0..4 {
            for i in 1..56 {
                seed_array[i] = seed_array[i].wrapping_sub(seed_array[1 + (i + 30) % 55]);
                if seed_array[i] < 0 { seed_array[i] = seed_array[i].wrapping_add(MBIG); }
            }
        }
        Self { seed_array, inext: 0, inextp: 21 }
    }

    pub fn next(&mut self) -> i32 {
        self.inext += 1;
        if self.inext >= 56 { self.inext = 1; }
        self.inextp += 1;
        if self.inextp >= 56 { self.inextp = 1; }
        let mut value = self.seed_array[self.inext] - self.seed_array[self.inextp];
        if value == MBIG { value -= 1; }
        if value < 0 { value += MBIG; }
        self.seed_array[self.inext] = value;
        value
    }

    pub fn sample(&mut self) -> f64 {
        self.next() as f64 * (1.0 / MBIG as f64)
    }

    pub fn range(&mut self, min: i32, max: i32) -> i32 {
        let span = i64::from(max) - i64::from(min);
        let fraction = if span <= i64::from(i32::MAX) {
            self.sample()
        } else {
            let value = self.next();
            let signed = if self.next() % 2 == 0 { -value } else { value };
            (f64::from(signed) + f64::from(MBIG - 1)) / (2.0 * f64::from(MBIG) - 1.0)
        };
        ((fraction * span as f64) as i64 + i64::from(min)) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::DotNetRandom;

    #[test]
    fn seeded_values_match_original_mono() {
        let rows = include_str!("../tests/fixtures/mono2019_random.csv");
        let mut random = DotNetRandom::new(0);
        for row in rows.lines().skip(1) {
            let fields: Vec<_> = row.split(',').collect();
            let seed: i32 = fields[1].parse().unwrap();
            let index: usize = fields[2].parse().unwrap();
            if index == 0 {
                random = DotNetRandom::new(seed);
            }
            if fields[0] == "system_double" {
                let expected: f64 = fields[3].parse().unwrap();
                assert_eq!(random.sample().to_bits(), expected.to_bits(), "{row}");
                continue;
            }
            let expected: i32 = fields[3].parse().unwrap();
            let actual = match fields[0] {
                "system_next" => random.next(),
                "system_max" => random.range(0, 100),
                "system_range" => random.range(-100, 100),
                "system_wide" => random.range(i32::MIN, i32::MAX),
                kind => panic!("unexpected vector kind {kind}"),
            };
            assert_eq!(actual, expected, "{row}");
        }
    }
}

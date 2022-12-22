pub mod curve;
pub mod func;
pub mod interpolation;

use cpal::Sample;
use std::{borrow::Borrow, iter::Peekable, mem::replace, slice::Iter};

use crate::func::{Function, MultiPoly, Wave};

#[derive(Debug, Clone)]
pub struct Player<'a> {
    pack: PackedTimedWaves<'a>,
    time: i64,
    wakeup: i64,
    current: Vec<TimedWave<&'a [f32]>>,
}
impl<'a> Player<'a> {
    pub fn new(pack: PackedTimedWaves<'a>, time: i64, wakeup: i64) -> Self {
        Player {
            pack,
            time,
            wakeup,
            current: Vec::new(),
        }
    }
    //this actually doesn't work at all when the buffer runs out
    pub fn play<'b, N: Sample>(
        &mut self,
        output: &'b mut [N],
    ) -> Result<(), (TimedWavePacker, &'b mut [N])> {
        let mut current = replace(&mut self.current, Vec::new());
        let mut buffer = output;
        loop {
            match self.pack.deposit_current(current, self.time, self.wakeup) {
                Ok((c, next_pause)) => {
                    let start_time = self.time;
                    let valid_for = next_pause - start_time;
                    let cut = buffer.len().min(valid_for as usize);
                    current = c;
                    let (working, future) = buffer.split_at_mut(cut);
                    buffer = future;
                    self.time += cut as i64;
                    for (current_sample, time) in working.iter_mut().zip(start_time..) {
                        let sample_value = current.iter().map(|tw| tw.eval(time)).sum::<f32>();
                        *current_sample = Sample::from(&(sample_value as f32));
                    }
                    if buffer.len() == 0 {
                        self.current = current;
                        return Ok(());
                    }
                }
                Err(packer) => return Err((packer, buffer)),
            }
        }
    }
    pub fn current_time(&self) -> i64 {
        self.time
    }
}
#[test]
fn buffer_writing() {
    let wave = Wave {
        freq: &[1.][..],
        amp: &[0.25][..],
        phase: 0.25,
    };
    let waves: TimedWavePacker = [(0, 6), (5, 8), (7, 9), (8, 12)]
        .into_iter()
        .map(|(start, end)| TimedWave {
            start,
            end,
            wave: wave.clone(),
        })
        .collect();
    let waves = waves.get_pack().unwrap();
    let mut player = Player::new(waves, 0, 11);
    let mut playback = [0.; 7];
    player.play(&mut playback).unwrap();
    assert_eq!(playback, [0.25, 0.25, 0.25, 0.25, 0.25, 0.5, 0.25]);
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimedWave<T> {
    pub start: i64,
    pub end: i64,
    pub wave: Wave<T, T>,
}
impl<T: Borrow<[f32]>> TimedWave<T> {
    fn eval(&self, time: i64) -> f32 {
        let adjusted = time - self.start;
        self.wave.eval(adjusted as f32)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimedWavePacker {
    pub timings: Vec<(i64, i64)>,
    pub freq_coef: Vec<f32>,
    pub freq_runs: Vec<u8>,
    pub amp_coef: Vec<f32>,
    pub amp_runs: Vec<u8>,
    pub phases: Vec<f32>,
}
impl<'a> TimedWavePacker {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get_pack(&'a self) -> Option<PackedTimedWaves<'a>> {
        let TimedWavePacker {
            timings,
            freq_coef,
            freq_runs,
            amp_coef,
            amp_runs,
            phases,
        } = self;
        PackedTimedWaves::new(timings, freq_coef, freq_runs, amp_coef, amp_runs, phases)
    }
    pub fn bulk_generate<F: Iterator<Item = f32>, A: Iterator<Item = f32>>(
        &mut self,
        timings: impl Iterator<Item = (i64, i64)>,
        frequencies: impl Iterator<Item = F>,
        amplitudes: impl Iterator<Item = A>,
        phases: impl Iterator<Item = f32>,
    ) {
        self.timings.extend(timings);
        for freq_group in frequencies {
            let start_len = self.freq_coef.len();
            self.freq_coef.extend(freq_group);
            let end_len = self.freq_coef.len();
            self.freq_runs.push((end_len - start_len) as u8);
        }
        for amp_group in amplitudes {
            let start_len = self.amp_coef.len();
            self.amp_coef.extend(amp_group);
            let end_len = self.amp_coef.len();
            self.amp_runs.push((end_len - start_len) as u8);
        }
        self.phases.extend(phases);
    }
}
impl<T: Borrow<[f32]>> Extend<TimedWave<T>> for TimedWavePacker {
    fn extend<I: IntoIterator<Item = TimedWave<T>>>(&mut self, iter: I) {
        for TimedWave { start, end, wave } in iter {
            let timing = (start, end);
            self.timings.push(timing);
            let Wave { freq, amp, phase } = wave;
            let (freq, amp) = (freq.borrow(), amp.borrow());
            let f_len = freq.len() as u8;
            let a_len = freq.len() as u8;
            self.freq_coef.extend_from_slice(freq);
            self.amp_coef.extend_from_slice(amp);
            self.freq_runs.push(f_len);
            self.amp_runs.push(a_len);
            self.phases.push(phase);
        }
    }
}
impl<T: Borrow<[f32]>> FromIterator<TimedWave<T>> for TimedWavePacker {
    fn from_iter<I: IntoIterator<Item = TimedWave<T>>>(iter: I) -> Self {
        let mut def = Self::new();
        def.extend(iter);
        def
    }
}

#[derive(Debug, Clone)]
pub struct PackedTimedWaves<'a> {
    timings: Peekable<Iter<'a, (i64, i64)>>,
    frequencies: MultiPoly<'a>,
    amplitudes: MultiPoly<'a>,
    phases: Iter<'a, f32>,
}
impl<'a, 's> PackedTimedWaves<'a> {
    pub fn new(
        timings: &'a [(i64, i64)],
        frequency_coef: &'a [f32],
        frequency_runs: &'a [u8],
        amplitude_coef: &'a [f32],
        amplitude_runs: &'a [u8],
        phases: &'a [f32],
    ) -> Option<Self> {
        ((timings.len() == frequency_runs.len())
            && (frequency_runs.len() == amplitude_runs.len())
            && (phases.len() == timings.len())
            && timings.windows(2).all(|s| s[0].0 <= s[1].0))
        .then_some(Self {
            timings: timings.iter().peekable(),
            frequencies: MultiPoly::new(frequency_coef, frequency_runs)?,
            amplitudes: MultiPoly::new(amplitude_coef, amplitude_runs)?,
            phases: phases.iter(),
        })
    }
    fn sample(&'s mut self, last_time: i64) -> WaveSlice<'s, 'a> {
        WaveSlice {
            waves: self,
            stop: last_time,
        }
    }
    fn unravel(self, current_store: Vec<TimedWave<&'a [f32]>>) -> TimedWavePacker {
        let mut packer = TimedWavePacker::new();
        packer.extend(current_store);
        {
            let MultiPoly {
                coeffs,
                run_lengths,
            } = self.amplitudes;
            packer.amp_coef.extend_from_slice(coeffs);
            packer.amp_runs.extend_from_slice(run_lengths.as_ref());
        }
        {
            let MultiPoly {
                coeffs,
                run_lengths,
            } = self.frequencies;
            packer.freq_coef.extend_from_slice(coeffs);
            packer.freq_runs.extend_from_slice(run_lengths.as_ref());
        }
        packer.phases.extend_from_slice(self.phases.as_slice());
        packer.timings.extend(self.timings);
        packer
    }
    fn deposit_current(
        &mut self,
        mut current_store: Vec<TimedWave<&'a [f32]>>,
        time: i64,
        wakeup_time: i64,
    ) -> Result<(Vec<TimedWave<&'a [f32]>>, i64), TimedWavePacker> {
        current_store.retain(|tw| tw.end > time);
        if time >= wakeup_time {
            let capture = replace(self, Self::default());
            return Err(capture.unravel(current_store));
        }
        current_store.extend(self.sample(time));

        let kill_wakeup_time = current_store
            .iter()
            .map(|tw| tw.end)
            .min()
            .unwrap_or(wakeup_time);
        let birth_wakeup_time = self.timings.peek().map(|&&(s, _)| s).unwrap_or(wakeup_time);
        let real_wakeup = kill_wakeup_time.min(birth_wakeup_time).min(wakeup_time);

        Ok((current_store, real_wakeup))
    }
}
impl<'a> Default for PackedTimedWaves<'a> {
    fn default() -> Self {
        Self::new(&[], &[], &[], &[], &[], &[]).unwrap()
    }
}

#[test]
fn depositing() {
    let waves: TimedWavePacker = [(0, 6), (5, 8), (7, 9), (8, 12)]
        .into_iter()
        .map(|(start, end)| TimedWave {
            start,
            end,
            wave: Wave::default(),
        })
        .collect();
    let mut waves = waves.get_pack().unwrap();
    let deposit = match waves.deposit_current(Vec::new(), 0, 8) {
        Ok((d, 5)) => d,
        Ok((_, n)) => panic!("next pause was {} insead of 5", n),
        Err(_) => panic!("failed to deposit"),
    };
    assert_eq!(
        deposit,
        vec![TimedWave {
            start: 0,
            end: 6,
            wave: Wave::default()
        }]
    );

    let deposit = match waves.deposit_current(deposit, 5, 8) {
        Ok((d, 6)) => d,
        Ok((_, n)) => panic!("next pause was {} insead of 6", n),
        Err(_) => panic!("failed to deposit"),
    };
    assert_eq!(
        deposit,
        vec![
            TimedWave {
                start: 0,
                end: 6,
                wave: Wave::default()
            },
            TimedWave {
                start: 5,
                end: 8,
                wave: Wave::default()
            }
        ]
    );

    let deposit = match waves.deposit_current(deposit, 6, 8) {
        Ok((d, 7)) => d,
        Ok((_, n)) => panic!("next pause was {} insead of 7", n),
        Err(_) => panic!("failed to deposit"),
    };
    assert_eq!(
        deposit,
        vec![TimedWave {
            start: 5,
            end: 8,
            wave: Wave::default()
        }]
    );

    let deposit = match waves.deposit_current(deposit, 7, 8) {
        Ok((d, 8)) => d,
        Ok((_, n)) => panic!("next pause was {} insead of 8", n),
        Err(_) => panic!("failed to deposit"),
    };
    assert_eq!(
        deposit,
        vec![
            TimedWave {
                start: 5,
                end: 8,
                wave: Wave::default()
            },
            TimedWave {
                start: 7,
                end: 9,
                wave: Wave::default()
            }
        ]
    );

    let packer = match waves.deposit_current(deposit, 8, 8) {
        Err(p) => p,
        Ok(_) => panic!("deposit failed to abort"),
    };
    let correct_packer: TimedWavePacker = [(7, 9), (8, 12)]
        .into_iter()
        .map(|(start, end)| TimedWave {
            start,
            end,
            wave: Wave::default(),
        })
        .collect();
    assert_eq!(packer, correct_packer);
}

#[derive(Debug)]
struct WaveSlice<'w, 's> {
    waves: &'w mut PackedTimedWaves<'s>,
    stop: i64,
}
impl<'w, 's> Iterator for WaveSlice<'w, 's> {
    type Item = TimedWave<&'s [f32]>;

    fn next(&mut self) -> Option<Self::Item> {
        let (start, end) = *self.waves.timings.next_if(|&&(s, _e)| s <= self.stop)?;
        let wave = (&mut self.waves.phases)
            .zip(&mut self.waves.frequencies)
            .zip(&mut self.waves.amplitudes)
            .next()
            .map(|((&phase, freq), amp)| Wave { freq, amp, phase })?;
        Some(TimedWave { start, end, wave })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.waves.phases.size_hint().1)
    }
}

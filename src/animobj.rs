use std::{
    ops::{Add, Mul},
    sync::Arc,
};

/// represents a virtual function on the interval [0, length]
#[derive(Debug, Clone)]
pub struct Anim<T: Copy + Sized + 'static> {
    length: f32,
    fn_handle: AnimFnHandle<T>,
}
/// utility newtype (to hang Debug on)
#[derive(Clone)]
struct AnimFnHandle<T>(Arc<dyn Fn(f32) -> T>);
impl<T> std::fmt::Debug for AnimFnHandle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AnimFnHandle(Arc<dyn Fn(f32) -> {}>)",
            std::any::type_name::<T>()
        )
    }
}
impl<T> AnimFnHandle<T> {
    fn call(&self, val: f32) -> T {
        (self.0)(val)
    }
}

// -- basic properties --
impl<T: Copy + Sized + 'static> Anim<T> {
    pub fn length(&self) -> f32 {
        self.length
    }
    pub fn sample(&self, t: f32) -> T {
        if t < 0.0 {
            self.fn_handle.call(0.0)
        } else if 0.0 <= t && t < self.length {
            self.fn_handle.call(t)
        } else {
            self.fn_handle.call(self.length)
        }
    }
    pub fn sample_looped(&self, t: f32) -> T {
        self.fn_handle.call(t % self.length)
    }
}

// -- builders --
impl<T: Copy + Sized + 'static> Anim<T> {
    pub fn func<F: Fn(f32) -> T + 'static>(length: f32, func: F) -> Self {
        Self {
            length,
            fn_handle: AnimFnHandle(Arc::new(func)),
        }
    }
    pub fn constant(v: T) -> Self {
        Self::func(1.0, move |_| v)
    }
    pub fn default() -> Self
    where
        T: Default,
    {
        Self::func(1.0, |_| T::default())
    }
    pub fn lerp(a: T, b: T) -> Anim<T>
    where
        T: Add<Output = T> + Mul<f32, Output = T>,
    {
        Self::func(1.0, move |t| a * (1.0 - t) + b * t)
    }
    /// returns None if anims is empty
    pub fn join(anims: &[Anim<T>]) -> Option<Anim<T>> {
        if anims.is_empty() {
            return None;
        }

        let total_length = anims.iter().fold(0.0, |acc, el| acc + el.length());
        let anims_cloned: Vec<_> = anims.iter().cloned().collect();

        let joined_anim = Self::func(total_length, move |t| {
            let mut next_t = 0.0f32;
            for anim in &anims_cloned {
                if next_t <= t && t < next_t + anim.length {
                    return anim.fn_handle.call(t - next_t);
                }
                next_t += anim.length;
            }
            if let Some(last_anim) = anims_cloned.last() {
                last_anim.fn_handle.call(last_anim.length)
            } else {
                unreachable!()
            }
        });

        Some(joined_anim)
    }
}

// -- combinators --
impl<T: Copy + Sized + 'static> Anim<T> {
    pub fn map<T2, F>(&self, f: F) -> Anim<T2>
    where
        T2: Copy + Sized + 'static,
        F: Fn(T) -> T2 + 'static,
    {
        let self_clone = self.clone();
        Anim::func(self_clone.length, move |t| f(self_clone.fn_handle.call(t)))
    }
    pub fn map_indexed<T2, F>(&self, f: F) -> Anim<T2>
    where
        T2: Copy + Sized + 'static,
        F: Fn(f32, T) -> T2 + 'static,
    {
        let self_clone = self.clone();
        Anim::func(self_clone.length, move |t| {
            f(t, self_clone.fn_handle.call(t))
        })
    }
    pub fn zip<T2>(&self, other: Anim<T2>) -> Anim<(T, T2)>
    where
        T2: Copy + Sized + 'static,
    {
        let self_clone = self.clone();
        let other_clone = other.clone();
        Anim::func(self_clone.length, move |t| {
            (self_clone.fn_handle.call(t), other_clone.fn_handle.call(t))
        })
    }
    pub fn zip_indexed<T2, F>(&self, other: Anim<T2>) -> Anim<(f32, T, T2)>
    where
        T2: Copy + Sized + 'static,
    {
        let self_clone = self.clone();
        let other_clone = other.clone();
        Anim::func(self_clone.length, move |t| {
            (
                t,
                self_clone.fn_handle.call(t),
                other_clone.fn_handle.call(t),
            )
        })
    }
    pub fn zip_map<T2, TOut, F>(&self, other: Anim<T2>, f: F) -> Anim<TOut>
    where
        T2: Copy + Sized + 'static,
        TOut: Copy + Sized + 'static,
        F: Fn(T, T2) -> TOut + 'static,
    {
        let self_clone = self.clone();
        let other_clone = other.clone();
        Anim::func(self_clone.length, move |t| {
            f(self_clone.fn_handle.call(t), other_clone.fn_handle.call(t))
        })
    }
    pub fn zip_map_indexed<T2, TOut, F>(&self, other: Anim<T2>, f: F) -> Anim<TOut>
    where
        T2: Copy + Sized + 'static,
        TOut: Copy + Sized + 'static,
        F: Fn(f32, T, T2) -> TOut + 'static,
    {
        let self_clone = self.clone();
        let other_clone = other.clone();
        Anim::func(self_clone.length, move |t| {
            f(
                t,
                self_clone.fn_handle.call(t),
                other_clone.fn_handle.call(t),
            )
        })
    }
    /// f should be a function from [0, 1] -> [0, 1]
    pub fn warp<F>(&self, f: F) -> Anim<T>
    where
        F: Fn(f32) -> f32 + 'static,
    {
        let self_clone = self.clone();
        Anim::func(self_clone.length, move |t| {
            self_clone
                .fn_handle
                .call(f(t / self_clone.length) * self_clone.length)
        })
    }
    pub fn reversed(&self) -> Anim<T> {
        let self_clone = self.clone();
        Anim::func(self_clone.length, move |t| {
            self_clone.fn_handle.call(self_clone.length - t)
        })
    }
    pub fn then(&self, other: &Anim<T>) -> Anim<T> {
        let self_clone = self.clone();
        let other_clone = other.clone();
        Anim::func(self.length + other.length, move |t| {
            if t <= self_clone.length {
                self_clone.fn_handle.call(t)
            } else {
                other_clone.fn_handle.call(t)
            }
        })
    }
    pub fn stretched(&self, factor: f32) -> Anim<T> {
        let self_clone = self.clone();
        Anim::func(self.length * factor, move |t| {
            self_clone.fn_handle.call(t / factor)
        })
    }
    pub fn delayed(&self, delay_length: f32) -> Anim<T> {
        let self_clone = self.clone();
        Anim::func(delay_length + self.length, move |t| {
            if t <= delay_length {
                self_clone.fn_handle.call(0.0)
            } else {
                self_clone.fn_handle.call(t - delay_length)
            }
        })
    }
    pub fn then_pause(&self, pause_length: f32) -> Anim<T> {
        let self_clone = self.clone();
        Anim::func(self.length + pause_length, move |t| {
            if t <= self_clone.length {
                self_clone.fn_handle.call(t)
            } else {
                self_clone.fn_handle.call(self_clone.length)
            }
        })
    }
}

pub mod f32 {
    use super::Anim;
    pub fn parabola() -> Anim<f32> {
        Anim::func(1.0, |t| 4.0 * t * (1.0 - t))
    }
    pub fn cubic_ease() -> Anim<f32> {
        Anim::func(1.0, |t| t * t * (3.0 - 2.0 * t))
    }
}

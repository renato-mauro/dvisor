use std::cmp::Ordering;

pub trait CastFrom<S>: Sized {
    fn cast_from(value: S) -> Self;
}

macro_rules! impl_cast_from {
    ($src:ty => $($dst:ty),+) => {
        $(impl CastFrom<$src> for $dst {
            #[inline]
            fn cast_from(value: $src) -> Self {
                value as $dst
            }
        })+
    };
}

impl_cast_from!(i8 => i64, i32, i16, i8);
impl_cast_from!(i16 => i64, i32, i16, i8);
impl_cast_from!(i32 => i64, i32, i16, i8, f32);
impl_cast_from!(i64 => i64, i32, i16, i8);
impl_cast_from!(f32 => f32, f64);
impl_cast_from!(f64 => f32, f64);

pub trait CmpI32 {
    fn cmp_i32(self, rhs: i32) -> Ordering;

    #[inline] fn lt(self, rhs: i32) -> bool where Self: Sized 
    { 
        self.cmp_i32(rhs) == Ordering::Less 
    }
    
    #[inline] fn le(self, rhs: i32) -> bool where Self: Sized 
    { 
        self.cmp_i32(rhs) != Ordering::Greater 
    }
    
    #[inline] fn gt(self, rhs: i32) -> bool where Self: Sized 
    { 
        self.cmp_i32(rhs) == Ordering::Greater 
    }
    
    #[inline] fn ge(self, rhs: i32) -> bool where Self: Sized 
    { 
        self.cmp_i32(rhs) != Ordering::Less 
    }
    
    #[inline] fn eq(self, rhs: i32) -> bool where Self: Sized 
    {
        self.cmp_i32(rhs) == Ordering::Equal 
    }

    #[inline] fn ne(self, rhs: i32) -> bool where Self: Sized 
    {
        self.cmp_i32(rhs) != Ordering::Equal
    }
}

impl CmpI32 for i8 {
    #[inline] fn cmp_i32(self, rhs: i32) -> Ordering {
        (self as i32).cmp(&rhs)
    }
}

impl CmpI32 for i16 {
    #[inline] fn cmp_i32(self, rhs: i32) -> Ordering {
        (self as i32).cmp(&rhs)
    }
}

impl CmpI32 for i32 {
    #[inline] fn cmp_i32(self, rhs: i32) -> Ordering {
        self.cmp(&rhs)
    }
}

impl CmpI32 for i64 {
    #[inline] fn cmp_i32(self, rhs: i32) -> Ordering {
        self.cmp(&(rhs as i64))
    }
}

impl CmpI32 for f32 {
    #[inline] fn cmp_i32(self, rhs: i32) -> Ordering {
        self.partial_cmp(&(rhs as f32)).unwrap_or(Ordering::Less)
    }
}

impl CmpI32 for f64 {
    #[inline] fn cmp_i32(self, rhs: i32) -> Ordering {
        self.partial_cmp(&(rhs as f64)).unwrap_or(Ordering::Less)
    }
}

pub trait CmpF32 {
    fn cmp_f32(self, rhs: f32) -> Option<Ordering>;

    #[inline] fn lt(self, rhs: f32) -> bool where Self: Sized 
    { 
        matches!(self.cmp_f32(rhs), Some(Ordering::Less))        
    }
    
    #[inline] fn le(self, rhs: f32) -> bool where Self: Sized 
    { 
        matches!(self.cmp_f32(rhs), Some(Ordering::Less | Ordering::Equal))        
    }
    
    #[inline] fn gt(self, rhs: f32) -> bool where Self: Sized 
    { 
        matches!(self.cmp_f32(rhs), Some(Ordering::Greater))        
    }
    
    #[inline] fn ge(self, rhs: f32) -> bool where Self: Sized 
    { 
        matches!(self.cmp_f32(rhs), Some(Ordering::Greater | Ordering::Equal))
    }
    
    #[inline] fn eq(self, rhs: f32) -> bool where Self: Sized 
    {
        matches!(self.cmp_f32(rhs), Some(Ordering::Equal))
    }

    #[inline] fn ne(self, rhs: f32) -> bool where Self: Sized 
    {
        matches!(self.cmp_f32(rhs), Some(Ordering::Greater | Ordering::Less))
    }
}

impl CmpF32 for i8 {
    #[inline] fn cmp_f32(self, rhs: f32) -> Option<Ordering> {
        (self as f32).partial_cmp(&rhs)
    }
}

impl CmpF32 for i16 {
    #[inline] fn cmp_f32(self, rhs: f32) -> Option<Ordering> {
        (self as f32).partial_cmp(&rhs)
    }
}

impl CmpF32 for i32 {
    #[inline] fn cmp_f32(self, rhs: f32) -> Option<Ordering> {
        (self as f32).partial_cmp(&rhs)
    }
}

impl CmpF32 for i64 {
    #[inline] fn cmp_f32(self, rhs: f32) -> Option<Ordering> {
        (self as f32).partial_cmp(&rhs)
    }
}

impl CmpF32 for f32 {
    #[inline] fn cmp_f32(self, rhs: f32) -> Option<Ordering> {
        self.partial_cmp(&rhs)
    }
}

impl CmpF32 for f64 {
    #[inline] fn cmp_f32(self, rhs: f32) -> Option<Ordering> {
        (self as f32).partial_cmp(&rhs)
    }
}

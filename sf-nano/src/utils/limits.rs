use core::fmt;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    min: usize,
    max: Option<usize>,
    default_max: Option<usize>,
    pub is64: bool,
}

#[derive(Debug)]
pub enum LimitsError {
    MinLargerThanMax,
    MaxLargerThanDefaultMax,
    MinLargerThanDefaultMax,
}

impl fmt::Display for LimitsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LimitsError::MinLargerThanMax => write!(f, "MinLargerThanMax"),
            LimitsError::MaxLargerThanDefaultMax => write!(f, "MaxLargerThanDefaultMax"),
            LimitsError::MinLargerThanDefaultMax => write!(f, "MinLargerThanDefaultMax"),
        }
    }
}

impl Limits {
    pub fn new(min: usize, max: Option<usize>) -> Result<Self, LimitsError> {
        if let Some(max) = max {
            if max < min {
                return Err(LimitsError::MinLargerThanMax);
            }
        }
        Ok(Limits {
            min,
            max,
            default_max: None,
            is64: false,
        })
    }

    pub fn new_64(min: usize, max: Option<usize>) -> Result<Self, LimitsError> {
        if let Some(max) = max {
            if max < min {
                return Err(LimitsError::MinLargerThanMax);
            }
        }
        Ok(Limits {
            min,
            max,
            default_max: None,
            is64: true,
        })
    }

    /// Returns the minimum value.
    pub fn min(&self) -> usize {
        self.min
    }

    /// Returns the explicit maximum value, if any.
    pub fn max(&self) -> Option<usize> {
        self.max
    }

    /// Returns the effective maximum (explicit max or default max).
    pub fn get_max(&self) -> usize {
        self.max
            .unwrap_or_else(|| self.default_max.unwrap_or(usize::MAX))
    }

    pub fn with_default_max(&self, default_max: usize) -> Result<Self, LimitsError> {
        if let Some(max) = self.max {
            if max > default_max {
                return Err(LimitsError::MaxLargerThanDefaultMax);
            }
        }
        if self.min > default_max {
            return Err(LimitsError::MinLargerThanDefaultMax);
        }
        Ok(Limits {
            min: self.min,
            max: self.max,
            default_max: Some(default_max),
            is64: self.is64,
        })
    }
}

pub trait Limitable {
    fn limits(&self) -> &Limits;

    fn get_min(&self) -> usize {
        self.limits().min
    }

    fn get_max(&self) -> usize {
        self.limits()
            .max
            .unwrap_or(self.limits().default_max.unwrap())
    }
}

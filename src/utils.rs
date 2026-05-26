use rand::distr::{Distribution, StandardUniform};
use rand::RngExt;

pub fn random_nr<T>() -> T
where
    StandardUniform: Distribution<T>,
{
     rand::rng().random()
}

fn to_x11_aspect_ratio(aspect_ratio: crate::AspectRatio) -> AspectRatio {
    match (aspect_ratio.numerator.try_into(), aspect_ratio.denominator.try_into()) {
        (Ok(numerator), Ok(denominator)) => AspectRatio { numerator, denominator },
        _ => AspectRatio {
            numerator: (aspect_ratio.numerator / 2) as i32,
            denominator: (aspect_ratio.denominator / 2) as i32,
        },
    }
}

/// A fully qualified identifier for resources, usually in `namespace:path` form.
pub type ResourceLocation = String;

/// Converts a type into a `ResourceLocation`.
pub trait ToResourceLocation: Sized {
    /// Converts the current instance into a `ResourceLocation`.
    ///
    /// # Returns
    /// A `String` representing the fully qualified resource identifier.
    fn to_resource_location(&self) -> ResourceLocation;
}

/// Constructs a type from a `ResourceLocation`.
pub trait FromResourceLocation: Sized {
    /// Attempts to create an instance from the given `ResourceLocation`.
    ///
    /// # Arguments
    /// * `resource_location` - The resource identifier to parse.
    ///
    /// # Returns
    /// `Some(Self)` if parsing succeeds, or `None` if the input is invalid.
    fn from_resource_location(resource_location: &ResourceLocation) -> Option<Self>;
}

/// Parse Java's Identifier syntax, supplying its default namespace.
pub fn normalize_identifier(value: &str) -> Option<String> {
    let (namespace, path) = value.split_once(':').unwrap_or(("minecraft", value));
    let namespace = if namespace.is_empty() {
        "minecraft"
    } else {
        namespace
    };
    let common =
        |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'-');
    if !namespace.bytes().all(common) || !path.bytes().all(|b| common(b) || b == b'/') {
        return None;
    }
    Some(format!("{namespace}:{path}"))
}

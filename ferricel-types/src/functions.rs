use std::fmt;

use strum::IntoEnumIterator;

/// Enumeration of all functions exported by the runtime WASM module
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIter)]
pub enum RuntimeFunction {
    // Memory
    Malloc,
    Free,

    // Arithmetic (Polymorphic)
    ValueAdd,
    ValueSub,
    ValueMul,
    ValueDiv,
    ValueMod,
    ValueNegate,

    // Arithmetic (Type-specific)
    DoubleAdd,
    DoubleSub,
    DoubleMul,
    DoubleDiv,
    IntDiv,
    IntMod,
    IntMul,
    IntSub,
    UintAdd,
    UintDiv,
    UintMod,
    UintMul,
    UintSub,

    // Comparison (Polymorphic)
    ValueEq,
    ValueNe,
    ValueGt,
    ValueLt,
    ValueGte,
    ValueLte,

    // Comparison (Type-specific)
    DoubleEq,
    DoubleGt,
    DoubleGte,
    DoubleLt,
    DoubleLte,
    DoubleNe,
    IntEq,
    IntGt,
    IntGte,
    IntLt,
    IntLte,
    IntNe,
    UintEq,
    UintGt,
    UintGte,
    UintLt,
    UintLte,
    UintNe,

    // Duration Comparison
    DurationGt,
    DurationGte,
    DurationLt,
    DurationLte,

    // Timestamp Comparison
    TimestampGt,
    TimestampGte,
    TimestampLt,
    TimestampLte,

    // Logical
    BoolAnd,
    BoolOr,
    BoolNot,
    NotStrictlyFalse,
    Conditional,
    IsStrictlyFalse,
    IsStrictlyTrue,
    IsError,
    IsBoolOrError,

    // Serialization
    SerializeValue,

    // Deserialization
    DeserializeJson,
    DeserializeProto,

    // Globals
    InitBindings,
    GetVariable,

    // Field Access
    GetField,
    HasField,

    // Array
    ArrayLen,
    ArrayGet,
    CreateArray,
    ArrayPush,

    // Map
    CreateMap,
    MapInsert,

    // Value Creation Helpers
    CreateInt,
    CreateUint,
    CreateBool,
    CreateDouble,
    CreateString,
    CreateBytes,
    CreateNull,
    CreateType,
    CreateError,

    // String Operations
    ValueSize,
    StringStartsWith,
    StringEndsWith,
    StringContains,
    StringMatches,
    StringCharAt,
    StringIndexOf,
    StringIndexOfOffset,
    StringLastIndexOf,
    StringLastIndexOfOffset,
    /// Polymorphic indexOf: dispatches on receiver type (string → substring search, list → element search)
    IndexOfPoly,
    /// Polymorphic lastIndexOf: dispatches on receiver type
    LastIndexOfPoly,
    StringLowerAscii,
    StringUpperAscii,
    StringReplace,
    StringReplaceN,
    StringSplit,
    StringSplitN,
    StringSubstring,
    StringSubstringRange,
    StringTrim,
    StringReverse,
    StringFormat,
    StringsQuote,

    // Encoders extension
    Base64Encode,
    Base64Decode,

    // Math extension
    MathGreatest,
    MathLeast,
    MathCeil,
    MathFloor,
    MathRound,
    MathTrunc,
    MathAbs,
    MathSign,
    MathIsInf,
    MathIsNaN,
    MathIsFinite,
    MathBitOr,
    MathBitAnd,
    MathBitXor,
    MathBitNot,
    MathBitShiftLeft,
    MathBitShiftRight,
    MathSqrt,

    // Regex extension
    RegexReplace,
    RegexReplaceN,
    RegexExtract,
    RegexExtractAll,

    ListJoin,
    ListJoinSep,

    // Membership
    ValueIn,

    // Index
    ValueIndex,

    // Conversions
    ValueToBool,
    ValueToI64,
    ValueToU64,

    // Type conversions / Constructor functions
    String,
    Int,
    Uint,
    Double,
    Bytes,
    Bool,
    Type,
    Duration,
    Timestamp,

    // Extension calls (fixed-arity wrappers for host-provided functions)
    ExtCall0,
    ExtCall1,
    ExtCall2,
    ExtCall3,
    ExtCall4,

    // Kubernetes List Extensions
    K8sListIsSorted,
    K8sListSum,
    K8sListMin,
    K8sListMax,
    K8sListIndexOf,
    K8sListLastIndexOf,

    // Kubernetes Regex Extensions
    K8sRegexFind,
    K8sRegexFindAllN,

    // Kubernetes URL Extensions
    K8sUrlParse,
    K8sIsUrl,
    K8sUrlGetScheme,
    K8sUrlGetHost,
    K8sUrlGetHostname,
    K8sUrlGetPort,
    K8sUrlGetEscapedPath,
    K8sUrlGetQuery,

    // Kubernetes IP Address Extensions
    K8sIpParse,
    K8sIsIp,
    K8sIpIsCanonical,
    K8sIpFamily,
    K8sIpIsUnspecified,
    K8sIpIsLoopback,
    K8sIpIsLinkLocalMulticast,
    K8sIpIsLinkLocalUnicast,
    K8sIpIsGlobalUnicast,

    // Kubernetes CIDR Extensions
    K8sCidrParse,
    K8sIsCidr,
    K8sCidrIp,
    K8sCidrMasked,
    K8sCidrPrefixLength,
    K8sCidrContainsIpObj,
    K8sCidrContainsIpStr,
    K8sCidrContainsCidrObj,
    K8sCidrContainsCidrStr,

    // Kubernetes Semver Extensions
    K8sSemverIsSemver,
    K8sSemverIsSemverNormalize,
    K8sSemverParse,
    K8sSemverParseNormalize,
    K8sSemverMajor,
    K8sSemverMinor,
    K8sSemverPatch,
    K8sSemverIsLessThan,
    K8sSemverIsGreaterThan,
    K8sSemverCompareTo,

    // Kubernetes Quantity Extensions
    K8sQuantityParse,
    K8sIsQuantity,
    K8sQuantitySign,
    K8sQuantityIsInteger,
    K8sQuantityAsInteger,
    K8sQuantityAsApproxFloat,
    K8sQuantityAdd,
    K8sQuantityAddInt,
    K8sQuantitySub,
    K8sQuantitySubInt,
    K8sQuantityIsLessThan,
    K8sQuantityIsGreaterThan,
    K8sQuantityCompareTo,

    // Kubernetes Polymorphic Comparisons (shared between Semver and Quantity)
    K8sPolyIsLessThan,
    K8sPolyIsGreaterThan,
    K8sPolyCompareTo,

    // CEL Optional type
    OptionalNone,
    OptionalOf,
    OptionalOfNonZeroValue,
    OptionalHasValue,
    OptionalValue,
    OptionalOrValue,
    OptionalOr,
    /// `receiver?.field` — optional field/key select (handles maps, objects, Optional wrappers)
    OptionalSelect,
    /// `container[?key]` — optional index (handles arrays, maps, Optional wrappers)
    OptionalIndex,

    // Kubernetes Format Extensions
    K8sFormatNamed,
    K8sFormatDns1123Label,
    K8sFormatDns1123Subdomain,
    K8sFormatDns1035Label,
    K8sFormatQualifiedName,
    K8sFormatDns1123LabelPrefix,
    K8sFormatDns1123SubdomainPrefix,
    K8sFormatDns1035LabelPrefix,
    K8sFormatLabelValue,
    K8sFormatUri,
    K8sFormatUuid,
    K8sFormatByte,
    K8sFormatDate,
    K8sFormatDatetime,
    K8sFormatValidate,

    // Timestamp accessors
    TimestampGetFullYear,
    TimestampGetFullYearTz,
    TimestampGetMonth,
    TimestampGetMonthTz,
    TimestampGetDate,
    TimestampGetDateTz,
    TimestampGetDayOfMonth,
    TimestampGetDayOfMonthTz,
    TimestampGetDayOfWeek,
    TimestampGetDayOfWeekTz,
    TimestampGetDayOfYear,
    TimestampGetDayOfYearTz,
    TimestampGetHours,
    TimestampGetHoursTz,
    TimestampGetMinutes,
    TimestampGetMinutesTz,
    TimestampGetSeconds,
    TimestampGetSecondsTz,
    TimestampGetMilliseconds,
    TimestampGetMillisecondsTz,
}

impl RuntimeFunction {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Malloc => "cel_malloc",
            Self::Free => "cel_free",

            Self::ValueAdd => "cel_value_add",
            Self::ValueSub => "cel_value_sub",
            Self::ValueMul => "cel_value_mul",
            Self::ValueDiv => "cel_value_div",
            Self::ValueMod => "cel_value_mod",
            Self::ValueNegate => "cel_value_negate",

            Self::DoubleAdd => "cel_double_add",
            Self::DoubleSub => "cel_double_sub",
            Self::DoubleMul => "cel_double_mul",
            Self::DoubleDiv => "cel_double_div",

            Self::IntDiv => "cel_int_div",
            Self::IntMod => "cel_int_mod",
            Self::IntMul => "cel_int_mul",
            Self::IntSub => "cel_int_sub",

            Self::UintAdd => "cel_uint_add",
            Self::UintDiv => "cel_uint_div",
            Self::UintMod => "cel_uint_mod",
            Self::UintMul => "cel_uint_mul",
            Self::UintSub => "cel_uint_sub",

            Self::ValueEq => "cel_value_eq",
            Self::ValueNe => "cel_value_ne",
            Self::ValueGt => "cel_value_gt",
            Self::ValueLt => "cel_value_lt",
            Self::ValueGte => "cel_value_gte",
            Self::ValueLte => "cel_value_lte",

            Self::DoubleEq => "cel_double_eq",
            Self::DoubleGt => "cel_double_gt",
            Self::DoubleGte => "cel_double_gte",
            Self::DoubleLt => "cel_double_lt",
            Self::DoubleLte => "cel_double_lte",
            Self::DoubleNe => "cel_double_ne",

            Self::IntEq => "cel_int_eq",
            Self::IntGt => "cel_int_gt",
            Self::IntGte => "cel_int_gte",
            Self::IntLt => "cel_int_lt",
            Self::IntLte => "cel_int_lte",
            Self::IntNe => "cel_int_ne",

            Self::UintEq => "cel_uint_eq",
            Self::UintGt => "cel_uint_gt",
            Self::UintGte => "cel_uint_gte",
            Self::UintLt => "cel_uint_lt",
            Self::UintLte => "cel_uint_lte",
            Self::UintNe => "cel_uint_ne",

            Self::DurationGt => "cel_duration_gt",
            Self::DurationGte => "cel_duration_gte",
            Self::DurationLt => "cel_duration_lt",
            Self::DurationLte => "cel_duration_lte",

            Self::TimestampGt => "cel_timestamp_gt",
            Self::TimestampGte => "cel_timestamp_gte",
            Self::TimestampLt => "cel_timestamp_lt",
            Self::TimestampLte => "cel_timestamp_lte",

            Self::BoolAnd => "cel_bool_and",
            Self::BoolOr => "cel_bool_or",
            Self::BoolNot => "cel_bool_not",
            Self::NotStrictlyFalse => "cel_not_strictly_false",
            Self::Conditional => "cel_conditional",
            Self::IsStrictlyFalse => "cel_is_strictly_false",
            Self::IsStrictlyTrue => "cel_is_strictly_true",
            Self::IsError => "cel_is_error",
            Self::IsBoolOrError => "cel_is_bool_or_error",

            Self::SerializeValue => "cel_serialize_value",

            Self::DeserializeJson => "cel_deserialize_json",
            Self::DeserializeProto => "cel_deserialize_proto",

            Self::InitBindings => "cel_init_bindings",
            Self::GetVariable => "cel_get_variable",

            Self::GetField => "cel_get_field",
            Self::HasField => "cel_has_field",

            Self::ArrayLen => "cel_array_len",
            Self::ArrayGet => "cel_array_get",
            Self::CreateArray => "cel_create_array",
            Self::ArrayPush => "cel_array_push",

            Self::CreateMap => "cel_create_map",
            Self::MapInsert => "cel_map_insert",

            Self::CreateInt => "cel_create_int",
            Self::CreateUint => "cel_create_uint",
            Self::CreateBool => "cel_create_bool",
            Self::CreateDouble => "cel_create_double",
            Self::CreateString => "cel_create_string",
            Self::CreateBytes => "cel_create_bytes",
            Self::CreateNull => "cel_create_null",
            Self::CreateType => "cel_create_type",
            Self::CreateError => "cel_create_error",

            Self::ValueSize => "cel_value_size",
            Self::StringStartsWith => "cel_string_starts_with",
            Self::StringEndsWith => "cel_string_ends_with",
            Self::StringContains => "cel_string_contains",
            Self::StringMatches => "cel_string_matches",
            Self::StringCharAt => "cel_string_char_at",
            Self::StringIndexOf => "cel_string_index_of",
            Self::StringIndexOfOffset => "cel_string_index_of_offset",
            Self::StringLastIndexOf => "cel_string_last_index_of",
            Self::StringLastIndexOfOffset => "cel_string_last_index_of_offset",
            Self::IndexOfPoly => "cel_index_of_poly",
            Self::LastIndexOfPoly => "cel_last_index_of_poly",
            Self::StringLowerAscii => "cel_string_lower_ascii",
            Self::StringUpperAscii => "cel_string_upper_ascii",
            Self::StringReplace => "cel_string_replace",
            Self::StringReplaceN => "cel_string_replace_n",
            Self::StringSplit => "cel_string_split",
            Self::StringSplitN => "cel_string_split_n",
            Self::StringSubstring => "cel_string_substring",
            Self::StringSubstringRange => "cel_string_substring_range",
            Self::StringTrim => "cel_string_trim",
            Self::StringReverse => "cel_string_reverse",
            Self::StringFormat => "cel_string_format",
            Self::StringsQuote => "cel_strings_quote",

            Self::Base64Encode => "cel_base64_encode",
            Self::Base64Decode => "cel_base64_decode",

            Self::MathGreatest => "cel_math_greatest",
            Self::MathLeast => "cel_math_least",
            Self::MathCeil => "cel_math_ceil",
            Self::MathFloor => "cel_math_floor",
            Self::MathRound => "cel_math_round",
            Self::MathTrunc => "cel_math_trunc",
            Self::MathAbs => "cel_math_abs",
            Self::MathSign => "cel_math_sign",
            Self::MathIsInf => "cel_math_is_inf",
            Self::MathIsNaN => "cel_math_is_nan",
            Self::MathIsFinite => "cel_math_is_finite",
            Self::MathBitOr => "cel_math_bit_or",
            Self::MathBitAnd => "cel_math_bit_and",
            Self::MathBitXor => "cel_math_bit_xor",
            Self::MathBitNot => "cel_math_bit_not",
            Self::MathBitShiftLeft => "cel_math_bit_shift_left",
            Self::MathBitShiftRight => "cel_math_bit_shift_right",
            Self::MathSqrt => "cel_math_sqrt",

            Self::RegexReplace => "cel_regex_replace",
            Self::RegexReplaceN => "cel_regex_replace_n",
            Self::RegexExtract => "cel_regex_extract",
            Self::RegexExtractAll => "cel_regex_extract_all",

            Self::ListJoin => "cel_list_join",
            Self::ListJoinSep => "cel_list_join_sep",

            Self::ValueIn => "cel_value_in",
            Self::ValueIndex => "cel_value_index",

            Self::ValueToBool => "cel_value_to_bool",
            Self::ValueToI64 => "cel_value_to_i64",
            Self::ValueToU64 => "cel_value_to_u64",

            Self::String => "cel_string",
            Self::Int => "cel_int",
            Self::Uint => "cel_uint",
            Self::Double => "cel_double",
            Self::Bytes => "cel_bytes",
            Self::Bool => "cel_bool",
            Self::Type => "cel_type",
            Self::Duration => "cel_duration",
            Self::Timestamp => "cel_timestamp",

            Self::ExtCall0 => "cel_ext_call_0",
            Self::ExtCall1 => "cel_ext_call_1",
            Self::ExtCall2 => "cel_ext_call_2",
            Self::ExtCall3 => "cel_ext_call_3",
            Self::ExtCall4 => "cel_ext_call_4",

            Self::K8sListIsSorted => "cel_k8s_list_is_sorted",
            Self::K8sListSum => "cel_k8s_list_sum",
            Self::K8sListMin => "cel_k8s_list_min",
            Self::K8sListMax => "cel_k8s_list_max",
            Self::K8sListIndexOf => "cel_k8s_list_index_of",
            Self::K8sListLastIndexOf => "cel_k8s_list_last_index_of",

            Self::K8sRegexFind => "cel_k8s_regex_find",
            Self::K8sRegexFindAllN => "cel_k8s_regex_find_all_n",

            Self::K8sUrlParse => "cel_k8s_url_parse",
            Self::K8sIsUrl => "cel_k8s_is_url",
            Self::K8sUrlGetScheme => "cel_k8s_url_get_scheme",
            Self::K8sUrlGetHost => "cel_k8s_url_get_host",
            Self::K8sUrlGetHostname => "cel_k8s_url_get_hostname",
            Self::K8sUrlGetPort => "cel_k8s_url_get_port",
            Self::K8sUrlGetEscapedPath => "cel_k8s_url_get_escaped_path",
            Self::K8sUrlGetQuery => "cel_k8s_url_get_query",

            Self::K8sIpParse => "cel_k8s_ip_parse",
            Self::K8sIsIp => "cel_k8s_is_ip",
            Self::K8sIpIsCanonical => "cel_k8s_ip_is_canonical",
            Self::K8sIpFamily => "cel_k8s_ip_family",
            Self::K8sIpIsUnspecified => "cel_k8s_ip_is_unspecified",
            Self::K8sIpIsLoopback => "cel_k8s_ip_is_loopback",
            Self::K8sIpIsLinkLocalMulticast => "cel_k8s_ip_is_link_local_multicast",
            Self::K8sIpIsLinkLocalUnicast => "cel_k8s_ip_is_link_local_unicast",
            Self::K8sIpIsGlobalUnicast => "cel_k8s_ip_is_global_unicast",

            Self::K8sCidrParse => "cel_k8s_cidr_parse",
            Self::K8sIsCidr => "cel_k8s_is_cidr",
            Self::K8sCidrIp => "cel_k8s_cidr_ip",
            Self::K8sCidrMasked => "cel_k8s_cidr_masked",
            Self::K8sCidrPrefixLength => "cel_k8s_cidr_prefix_length",
            Self::K8sCidrContainsIpObj => "cel_k8s_cidr_contains_ip_obj",
            Self::K8sCidrContainsIpStr => "cel_k8s_cidr_contains_ip_str",
            Self::K8sCidrContainsCidrObj => "cel_k8s_cidr_contains_cidr_obj",
            Self::K8sCidrContainsCidrStr => "cel_k8s_cidr_contains_cidr_str",

            Self::K8sSemverIsSemver => "cel_k8s_is_semver",
            Self::K8sSemverIsSemverNormalize => "cel_k8s_is_semver_normalize",
            Self::K8sSemverParse => "cel_k8s_semver_parse",
            Self::K8sSemverParseNormalize => "cel_k8s_semver_parse_normalize",
            Self::K8sSemverMajor => "cel_k8s_semver_major",
            Self::K8sSemverMinor => "cel_k8s_semver_minor",
            Self::K8sSemverPatch => "cel_k8s_semver_patch",
            Self::K8sSemverIsLessThan => "cel_k8s_semver_is_less_than",
            Self::K8sSemverIsGreaterThan => "cel_k8s_semver_is_greater_than",
            Self::K8sSemverCompareTo => "cel_k8s_semver_compare_to",

            Self::K8sQuantityParse => "cel_k8s_quantity_parse",
            Self::K8sIsQuantity => "cel_k8s_is_quantity",
            Self::K8sQuantitySign => "cel_k8s_quantity_sign",
            Self::K8sQuantityIsInteger => "cel_k8s_quantity_is_integer",
            Self::K8sQuantityAsInteger => "cel_k8s_quantity_as_integer",
            Self::K8sQuantityAsApproxFloat => "cel_k8s_quantity_as_approx_float",
            Self::K8sQuantityAdd => "cel_k8s_quantity_add",
            Self::K8sQuantityAddInt => "cel_k8s_quantity_add_int",
            Self::K8sQuantitySub => "cel_k8s_quantity_sub",
            Self::K8sQuantitySubInt => "cel_k8s_quantity_sub_int",
            Self::K8sQuantityIsLessThan => "cel_k8s_quantity_is_less_than",
            Self::K8sQuantityIsGreaterThan => "cel_k8s_quantity_is_greater_than",
            Self::K8sQuantityCompareTo => "cel_k8s_quantity_compare_to",

            Self::K8sPolyIsLessThan => "cel_k8s_poly_is_less_than",
            Self::K8sPolyIsGreaterThan => "cel_k8s_poly_is_greater_than",
            Self::K8sPolyCompareTo => "cel_k8s_poly_compare_to",

            Self::OptionalNone => "cel_optional_none",
            Self::OptionalOf => "cel_optional_of",
            Self::OptionalOfNonZeroValue => "cel_optional_of_non_zero_value",
            Self::OptionalHasValue => "cel_optional_has_value",
            Self::OptionalValue => "cel_optional_value",
            Self::OptionalOrValue => "cel_optional_or_value",
            Self::OptionalOr => "cel_optional_or",
            Self::OptionalSelect => "cel_optional_select",
            Self::OptionalIndex => "cel_optional_index",

            Self::K8sFormatNamed => "cel_k8s_format_named",
            Self::K8sFormatDns1123Label => "cel_k8s_format_dns1123_label",
            Self::K8sFormatDns1123Subdomain => "cel_k8s_format_dns1123_subdomain",
            Self::K8sFormatDns1035Label => "cel_k8s_format_dns1035_label",
            Self::K8sFormatQualifiedName => "cel_k8s_format_qualified_name",
            Self::K8sFormatDns1123LabelPrefix => "cel_k8s_format_dns1123_label_prefix",
            Self::K8sFormatDns1123SubdomainPrefix => "cel_k8s_format_dns1123_subdomain_prefix",
            Self::K8sFormatDns1035LabelPrefix => "cel_k8s_format_dns1035_label_prefix",
            Self::K8sFormatLabelValue => "cel_k8s_format_label_value",
            Self::K8sFormatUri => "cel_k8s_format_uri",
            Self::K8sFormatUuid => "cel_k8s_format_uuid",
            Self::K8sFormatByte => "cel_k8s_format_byte",
            Self::K8sFormatDate => "cel_k8s_format_date",
            Self::K8sFormatDatetime => "cel_k8s_format_datetime",
            Self::K8sFormatValidate => "cel_k8s_format_validate",

            Self::TimestampGetFullYear => "cel_timestamp_get_full_year",
            Self::TimestampGetFullYearTz => "cel_timestamp_get_full_year_tz",
            Self::TimestampGetMonth => "cel_timestamp_get_month",
            Self::TimestampGetMonthTz => "cel_timestamp_get_month_tz",
            Self::TimestampGetDate => "cel_timestamp_get_date",
            Self::TimestampGetDateTz => "cel_timestamp_get_date_tz",
            Self::TimestampGetDayOfMonth => "cel_timestamp_get_day_of_month",
            Self::TimestampGetDayOfMonthTz => "cel_timestamp_get_day_of_month_tz",
            Self::TimestampGetDayOfWeek => "cel_timestamp_get_day_of_week",
            Self::TimestampGetDayOfWeekTz => "cel_timestamp_get_day_of_week_tz",
            Self::TimestampGetDayOfYear => "cel_timestamp_get_day_of_year",
            Self::TimestampGetDayOfYearTz => "cel_timestamp_get_day_of_year_tz",
            Self::TimestampGetHours => "cel_timestamp_get_hours",
            Self::TimestampGetHoursTz => "cel_timestamp_get_hours_tz",
            Self::TimestampGetMinutes => "cel_timestamp_get_minutes",
            Self::TimestampGetMinutesTz => "cel_timestamp_get_minutes_tz",
            Self::TimestampGetSeconds => "cel_timestamp_get_seconds",
            Self::TimestampGetSecondsTz => "cel_timestamp_get_seconds_tz",
            Self::TimestampGetMilliseconds => "cel_timestamp_get_milliseconds",
            Self::TimestampGetMillisecondsTz => "cel_timestamp_get_milliseconds_tz",
        }
    }

    /// Returns true if this function should be exported in the final WASM module
    pub fn is_exported(&self) -> bool {
        matches!(self, Self::Malloc | Self::Free)
    }

    /// Iterates over all variants
    pub fn iter() -> impl Iterator<Item = RuntimeFunction> {
        <RuntimeFunction as IntoEnumIterator>::iter()
    }
}

impl fmt::Display for RuntimeFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

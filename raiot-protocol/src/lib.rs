//! The Raiot-Protocol Crate

#![warn(
    absolute_paths_not_starting_with_crate,
    ambiguous_associated_items,
    anonymous_parameters,
    asm_sub_register,
    bare_trait_objects,
    bindings_with_variant_name,
    // box_pointers,
    coherence_leak_check,
    conflicting_repr_hints,
    confusable_idents,
    const_err,
    dead_code,
    deprecated_in_future,
    elided_lifetimes_in_paths,
    ellipsis_inclusive_range_patterns,
    explicit_outlives_requirements,
    exported_private_dependencies,
    ill_formed_attribute_input,
    illegal_floating_point_literal_pattern,
    improper_ctypes,
    indirect_structural_match,
    inline_no_sanitize,
    broken_intra_doc_links,
    invalid_codeblock_attributes,
    invalid_type_param_default,
    invalid_value,
    irrefutable_let_patterns,
    keyword_idents,
    late_bound_lifetime_arguments,
    macro_expanded_macro_exports_accessed_by_absolute_paths,
    macro_use_extern_crate,
    meta_variable_misuse,
    missing_copy_implementations,
    missing_crate_level_docs,
    missing_debug_implementations,
    missing_doc_code_examples,
    missing_docs,
    missing_fragment_specifier,
    mutable_borrow_reservation_conflict,
    mutable_transmutes,
    no_mangle_const_items,
    no_mangle_generic_items,
    non_ascii_idents,
    non_camel_case_types,
    non_shorthand_field_patterns,
    non_snake_case,
    non_upper_case_globals,
    order_dependent_trait_objects,
    overflowing_literals,
    overlapping_range_endpoints,
    path_statements,
    patterns_in_fns_without_body,
    private_doc_tests,
    proc_macro_derive_resolution_fallback,
    pub_use_of_private_extern_crate,
    redundant_semicolons,
    renamed_and_removed_lints,
    safe_packed_borrows,
    single_use_lifetimes,
    soft_unstable,
    stable_features,
    trivial_bounds,
    trivial_casts,
    trivial_numeric_casts,
    type_alias_bounds,
    tyvar_behind_raw_pointer,
    unaligned_references,
    uncommon_codepoints,
    unconditional_panic,
    unconditional_recursion,
    unknown_crate_types,
    unknown_lints,
    unnameable_test_items,
    unreachable_code,
    unreachable_patterns,
    unreachable_pub,
    unsafe_code,
    unstable_features,
    unstable_name_collisions,
    unused_allocation,
    unused_assignments,
    unused_attributes,
    unused_braces,
    unused_comparisons,
    unused_crate_dependencies,
    unused_doc_comments,
    unused_extern_crates,
    unused_features,
    unused_import_braces,
    unused_imports,
    unused_labels,
    unused_lifetimes,
    unused_macros,
    unused_must_use,
    unused_mut,
    unused_parens,
    unused_qualifications,
    unused_results,
    unused_unsafe,
    unused_variables,
    variant_size_differences,
    warnings,
    where_clauses_object_safety,
    while_true,
    arithmetic_overflow,
    array_into_iter,
    deprecated,
    incomplete_features,
    private_in_public,
)]

use log;
use serde_json;

/// Device and module identities
pub mod identity;

/// IoT protocol encoder/decoder
pub mod iot_codec;

/// The IoT Hub protocol messages
pub mod messages;

/// QoS, delivery guatantees and acknowledgements
pub mod qos;

/// Authentication methods
pub mod auth;

pub use crate::identity::*;
pub use crate::iot_codec::*;
pub use crate::messages::*;
pub use crate::subscription::*;

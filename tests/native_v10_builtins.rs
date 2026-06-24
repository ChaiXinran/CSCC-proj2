use agentjs::{
    backend::BackendKind,
    engine::{Engine, ExecutionOptions, RuntimeConfig},
};

fn native_eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
        .value
}

#[test]
fn date_constructor_static_methods_and_descriptors_are_installed() {
    assert_eq!(
        native_eval(
            "typeof Date + ':' + Date.name + ':' + Date.length + ':' + \
             Date.parse('1970-01-02T00:00:00.000Z') + ':' + Date.UTC(1970, 0, 2) + ':' + \
             Object.getOwnPropertyDescriptor(Date, 'prototype').writable;"
        ),
        "function:Date:7:86400000:86400000:false"
    );
}

#[test]
fn date_instances_support_utc_value_and_string_basics() {
    assert_eq!(
        native_eval(
            "var d = new Date(0); \
             d.getTime() + ':' + d.valueOf() + ':' + d.toISOString() + ':' + \
             d.getUTCFullYear() + ':' + d.getUTCMonth() + ':' + d.getUTCDate() + ':' + \
             d.getUTCDay() + ':' + d.getTimezoneOffset() + ':' + Object.prototype.toString.call(d);"
        ),
        "0:0:1970-01-01T00:00:00.000Z:1970:0:1:4:0:[object Date]"
    );
}

#[test]
fn invalid_dates_are_observable_without_panics() {
    assert_eq!(
        native_eval(
            "var d = new Date('not-a-date'); \
             var range = false; \
             try { d.toISOString(); } catch (e) { range = e.name === 'RangeError'; } \
             (d.getTime() !== d.getTime()) + ':' + d.toString() + ':' + (d.toJSON() === null) + ':' + range;"
        ),
        "true:Invalid Date:true:true"
    );
}

#[test]
fn intl_datetime_and_number_fallbacks_are_deterministic() {
    assert_eq!(
        native_eval(
            "var dtf = new Intl.DateTimeFormat(); \
             var parts = dtf.formatToParts(new Date(0)); \
             var nfp = Intl.NumberFormat().formatToParts(-12.5); \
             dtf.format(new Date(0)) + ':' + dtf.formatRange(0, 86400000) + ':' + \
             parts[0].type + parts[0].value + ':' + \
             nfp[0].type + nfp[0].value + ':' + nfp[1].type + nfp[1].value + ':' + nfp[3].type + nfp[3].value;"
        ),
        "1970-01-01:1970-01-01 - 1970-01-02:year1970:minusSign-:integer12:fraction5"
    );
}

#[test]
fn intl_additional_constructors_expose_basic_fallback_behavior() {
    assert_eq!(
        native_eval(
            "var pr = Intl.PluralRules(); \
             var rtf = new Intl.RelativeTimeFormat(); \
             var lf = new Intl.ListFormat(); \
             var loc = new Intl.Locale('EN-us'); \
             pr.select(1) + ':' + pr.select(2) + ':' + rtf.format(-1, 'day') + ':' + \
             lf.format(['a', 'b', 'c']) + ':' + loc.toString() + ':' + loc.language + ':' + \
             Intl.getCanonicalLocales(['EN-us', 'fr-fr']).join(',');"
        ),
        "one:other:1 day ago:a, b, and c:en-US:en:en-US,fr-FR"
    );
}

#[test]
fn temporal_core_types_expose_basic_construction_and_strings() {
    assert_eq!(
        native_eval(
            "var dur = new Temporal.Duration(0, 0, 0, 1, 2); \
             var inst = Temporal.Instant.fromEpochMilliseconds(0); \
             var date = Temporal.PlainDate.from('2020-05-02'); \
             var time = Temporal.PlainTime.from('12:34:56.789'); \
             var dateTime = Temporal.PlainDateTime.from('2020-05-02T12:34:56'); \
             dur.days + ':' + dur.hours + ':' + Temporal.Duration.from('P1DT2H').toString() + ':' + \
             inst.toString() + ':' + Temporal.Instant.compare(inst, Temporal.Instant.fromEpochMilliseconds(1)) + ':' + \
             date.toString() + ':' + time.toString() + ':' + dateTime.toString() + ':' + Temporal.Now.timeZoneId();"
        ),
        "1:2:P1DT2H:1970-01-01T00:00:00.000Z:-1:2020-05-02:12:34:56.789:2020-05-02T12:34:56:UTC"
    );
}

#[test]
fn temporal_constructor_calls_throw_explicit_type_errors() {
    assert_eq!(
        native_eval(
            "var caught = false; \
             try { Temporal.PlainDate(2020, 1, 1); } catch (e) { caught = e.name === 'TypeError'; } \
             caught;"
        ),
        "true"
    );
}

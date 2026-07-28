use agentjs::{Engine, ExecutionOptions, RuntimeConfig};

fn run(source: &str) -> String {
    Engine::new(RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .expect("native evaluation succeeds")
        .value
}

#[test]
fn plain_date_difference_rounds_relative_years_and_months() {
    assert_eq!(
        run(
            "let a = new Temporal.PlainDate(2019, 1, 8); let b = new Temporal.PlainDate(2021, 9, 7); let y = a.until(b, { smallestUnit: 'year', roundingMode: 'ceil' }); let m = a.until(b, { smallestUnit: 'month', roundingIncrement: 10, roundingMode: 'halfExpand' }); y.years + ':' + y.months + ':' + m.months"
        ),
        "3:0:30"
    );
}

#[test]
fn plain_date_time_rounding_includes_the_time_fraction() {
    assert_eq!(
        run(
            "let a = new Temporal.PlainDateTime(2019, 1, 8, 8); let b = new Temporal.PlainDateTime(2021, 9, 7, 12); let m = a.until(b, { smallestUnit: 'month', roundingMode: 'ceil' }); let d = a.until(b, { smallestUnit: 'day', roundingMode: 'ceil' }); m.months + ':' + d.days"
        ),
        "32:974"
    );
}

#[test]
fn temporal_add_rejects_results_outside_the_iso_date_range() {
    assert_eq!(
        run(
            "let count = 0; try { new Temporal.PlainDate(275760, 9, 13).add({ days: 1 }); } catch (e) { count += e instanceof RangeError; } try { new Temporal.PlainDateTime(-271821, 4, 19).subtract({ nanoseconds: 1 }); } catch (e) { count += e instanceof RangeError; } count"
        ),
        "2"
    );
}

#[test]
fn duration_add_and_subtract_reject_calendar_units_without_relative_to() {
    assert_eq!(
        run(
            "let blank = new Temporal.Duration(); let years = new Temporal.Duration(1); let count = 0; try { years.add(blank); } catch (e) { count += e instanceof RangeError; } try { blank.subtract({ months: 1 }); } catch (e) { count += e instanceof RangeError; } try { blank.add('P1W'); } catch (e) { count += e instanceof RangeError; } count"
        ),
        "3"
    );
}

#[test]
fn plain_year_month_difference_defaults_to_year_largest_unit() {
    assert_eq!(
        run(
            "let a = new Temporal.PlainYearMonth(2020, 2); let b = new Temporal.PlainYearMonth(2022, 5); let u = a.until(b); let s = b.since(a, { largestUnit: 'auto' }); u.years + ':' + u.months + ':' + s.years + ':' + s.months"
        ),
        "2:3:2:3"
    );
}

#[test]
fn plain_time_addition_preserves_exact_subseconds() {
    assert_eq!(
        run(
            "let t = new Temporal.PlainTime(0, 0, 0, 0, 0, 1); let r = t.add({ hours: 1000000000, nanoseconds: 568 }); r.nanosecond"
        ),
        "569"
    );
}

#[test]
fn zoned_date_time_difference_defaults_to_hours() {
    assert_eq!(
        run(
            "let a = new Temporal.ZonedDateTime(0n, 'UTC'); let b = new Temporal.ZonedDateTime(90000000000000n, 'UTC'); let d = a.until(b); d.days + ':' + d.hours"
        ),
        "0:25"
    );
}

#[test]
fn duration_addition_balances_to_the_largest_input_unit_exactly() {
    assert_eq!(
        run(
            "let a = new Temporal.Duration(); let h = a.add('-PT24.567890123H'); let d = new Temporal.Duration(0,0,0,1).add({ hours: 24 }); h.days + ':' + h.hours + ':' + h.nanoseconds + ':' + d.days"
        ),
        "0:-24:-800:2"
    );
}

#[test]
fn plain_year_month_arithmetic_rejects_lower_units_and_validates_options() {
    assert_eq!(
        run(
            "let ym = new Temporal.PlainYearMonth(2020, 1); let count = 0; try { ym.add({ days: 1 }); } catch (e) { count += e instanceof RangeError; } try { ym.subtract({ hours: 1 }); } catch (e) { count += e instanceof RangeError; } try { ym.add({}, null); } catch (e) { count += e instanceof TypeError; } count"
        ),
        "3"
    );
}

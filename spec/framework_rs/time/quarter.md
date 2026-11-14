# Quarter

The parts that apply to all of this feature are in `spec/framework_rs/time/context.md`.

## Public items

`ian_framework::time` exports these items:

| Item | Kind |
| --- | --- |
| `Quarter` | Struct |

## Quarter

`Quarter` has the public fields `year: i32` and `number: u32`. `number` is from 1 to 4. It derives `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd` and `Ord`. The order is by `year`, then by `number`.

| Method | Result |
| --- | --- |
| `Quarter::of(day: chrono::NaiveDate)` | The quarter that holds `day`. `number` is `(month - 1) / 3 + 1`. |
| `first_day(self) -> String` | The text `YYYY-MM-01` of the first month of the quarter. |
| `last_day(self) -> String` | The text `YYYY-MM-DD` of the last day of the quarter: `03-31`, `06-30`, `09-30` or `12-31`. |

- `Display` gives `YYYY-QN`, with the year in 4 digits.
- The method `first_day` and the method `last_day` do not check that `number` is from 1 to 4.

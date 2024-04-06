```{"t":"Data"}
name: hours
data:
- date: 2022-10-22
  hours: 1
- date: 2022-11-03
  hours: 2.5
- date: 2022-11-05
  hours: 5
- date: 2022-11-27
  hours: 2
- date: 2023-01-05
  hours: 2
- date: 2023-01-27
  hours: 6.5
- date: 2023-02-01
  hours: 2.5
- date: 2023-02-08
  hours: 6
```

```{"t":"Script","hidden_title":"Hours"}
let total_hours = 0;

for day in hours {
    total_hours += parse_float(day["hours"]);
}
```

I've spent `_total_hours_` on some project.

```{"t":"DynamicTable"}
row(["Month", "Hours"]);

let hours_by_month = #{};
for day in hours {
  let month = day["date"].sub_string(0, 7);

  let hours = parse_float(day["hours"]);

  if month in hours_by_month {
    hours_by_month[month] = hours_by_month[month] + hours;
  } else {
    hours_by_month[month] = hours;
  }
}

for month in hours_by_month.keys() {
  row([month, hours_by_month[month]]);
}
```

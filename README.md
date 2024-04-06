# Yet another markdown renderer (WIP)

This is a markdown renderer using/abusing [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) to get some additional features I wanted for my note-taking.

The main addition I wanted was some sort of interactivity/scripting for automation.
Basically a simpler version of Jupyter Notebook that works locally, where I don't need to view/run/build the document in a browser/app.

One thing I actually use this for is time tracking for different projects, so in the same markdown file where I have my notes, I can add something like this:

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

Then I can render an HTML version of the file using `yamdr-cli -f example.md render -`, and it will run the scripts and display the value of `total_hours`, and generate a table.

Or if I don’t want to view the results in a browser, but just in the same editor I’m using to take notes, I can just run (in vim):

```
:%! yamdr-cli -f % render --format md -
```

And the file will be replaced with this:

    ```{"t":"Data"}
    name: hours
    data:
    - date: 2022-10-22
      hours: '1'
    - date: 2022-11-03
      hours: '2.5'
    - date: 2022-11-05
      hours: '5'
    - date: 2022-11-27
      hours: '2'
    - date: 2023-01-05
      hours: '2'
    - date: 2023-01-27
      hours: '6.5'
    - date: 2023-02-01
      hours: '2.5'
    - date: 2023-02-08
      hours: '6'
    
    # | # | date | hours |
    # |---|---|---|
    # | 1 | 2022-10-22 | 1 |
    # | 2 | 2022-11-03 | 2.5 |
    # | 3 | 2022-11-05 | 5 |
    # | 4 | 2022-11-27 | 2 |
    # | 5 | 2023-01-05 | 2 |
    # | 6 | 2023-01-27 | 6.5 |
    # | 7 | 2023-02-01 | 2.5 |
    # | 8 | 2023-02-08 | 6 |
    ```
    
    ```{"t":"Script","hidden_title":"Hours"}
    let total_hours = 0;
    
    for day in hours {
        total_hours += parse_float(day["hours"]);
    }
    ```
    
    I’ve spent `_total_hours // > 27.5_` on some project.
    
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
    // > | Month | Hours |
    // > |---|---|
    // > | 2022-10 | 1.0 |
    // > | 2022-11 | 9.5 |
    // > | 2023-01 | 8.5 |
    // > | 2023-02 | 8.5 |
    ```

So I can see the results instantly. If I add or change something in the `hours` list, I can just rerun the command and see the updated values.

## Integrating with other stuff

I also wanted to be able to use this crate as a library to parse markdown files in other projects as well.
But other projects might need other types of custom blocks, for example if I'm rendering webpages, I might want to be able to define some metadata inside the file.
That way I can parse the file, get the rendered content, and some structured data about the file that I can use at "parse-time", for example related to templating.

For this, I added an "External"-type block that can contain additional fields in the "header", and where the header and body can be accessed after parsing the file.
Defining metadata about a page written in markdown may therefore look like this (with the `meta: true` indicating that this is a "meta block"):

    ```{t: External, meta: true}
    title: My page
    public: true
    navigation:
      href: /index.html
      title: Back to home
    css: |
      .page-specific-class {
        color: red;
      }
    ```

After parsing the file, I can check for external blocks that set `meta: true`, and (yaml) parse the content of that block to get the metadata about that file.

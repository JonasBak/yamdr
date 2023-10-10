# Yet another markdown renderer (WIP)

This is a markdown renderer using/abusing [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) to get some additional features I wanted for my note-taking.

The main addition I wanted was some sort of interactivity/scripting for automation. Basically Jupyter Notebook, but I wanted the scripting to work without needing to view the document in a browser/app.

One thing I actually use this for is time tracking for different projects, so in the same markdown file where I have my notes, I can add something like this:

    ```{"t":"Script","hidden_title":"Hours"}
    let hours = [
      ["2022-10-22",1],
      ["2022-11-03",2.5],
      ["2022-11-05",5],
      ["2022-11-27",2],
      ["2023-01-05",2],
      ["2023-01-27",6.5],
      ["2023-02-01",2.5],
      ["2023-02-08",6],
    ];
    
    let total_hours = 0;
    
    for day in hours {
        total_hours += day[1];
    }
    ```
    
    I've spent `_total_hours_` on some project.
    
    ```{"t":"DynamicTable"}
    row(["Month", "Hours"]);
    
    let hours_by_month = #{};
    for day in hours {
      let month = day[0].sub_string(0, 7);
    
      if month in hours_by_month {
        hours_by_month[month] = hours_by_month[month] + day[1];
      } else {
        hours_by_month[month] = day[1];
      }
    }
    
    for month in hours_by_month.keys() {
      row([month, hours_by_month[month]]);
    }
    ```

Then I can render an HTML version of the file using `yamdr-cli -f sync/sync/md/trening.md render`, and it will run the scripts and display the value of `total_hours`, and generate a table.

Or if I don’t want to view the results in a browser, but just in the same editor I’m using to take notes, I can just run (in vim):

```
:%! yamdr-cli -f % render --format md -
```

And the file will be replaced with this:

    ```{"t":"Script","hidden_title":"Hours"}
    let hours = [
      ["2022-10-22",1],
      ["2022-11-03",2.5],
      ["2022-11-05",5],
      ["2022-11-27",2],
      ["2023-01-05",2],
      ["2023-01-27",6.5],
      ["2023-02-01",2.5],
      ["2023-02-08",6],
    ];
    
    let total_hours = 0;
    
    for day in hours {
        total_hours += day[1];
    }
    ```
    
    I’ve spent `_total_hours // > 27.5_` on some project.
    
    ```{"t":"DynamicTable"}
    row(["Month", "Hours"]);
    
    let hours_by_month = #{};
    for day in hours {
      let month = day[0].sub_string(0, 7);
    
      if month in hours_by_month {
        hours_by_month[month] = hours_by_month[month] + day[1];
      } else {
        hours_by_month[month] = day[1];
      }
    }
    
    for month in hours_by_month.keys() {
      row([month, hours_by_month[month]]);
    }
    // > | Month | Hours |
    // > |---|---|
    // > | 2022-10 | 1 |
    // > | 2022-11 | 9.5 |
    // > | 2023-01 | 8.5 |
    // > | 2023-02 | 8.5 |
    ```

So I can see the results instantly. If I add or change something in the `hours` list, I can just rerun the command and see the updated values.



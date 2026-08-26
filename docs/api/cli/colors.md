By default, `page` uses color in the terminal output.

![Example of terminal output, with colors on some key words.](../../images/terminal-colors.png)

In order to follow the [NO_COLOR standard](https://no-color.org/). You can either set the `NO_COLOR` environment variable or pass `--no-color` to disable them:

```sh
page validate document.pdf --no-color
```

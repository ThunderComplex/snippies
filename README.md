# Snippies

Self-hostable code snippet solution

## What is snippies?

Snippies is a minimal solution to create your own self-hosted repository of code snippets.  
A snippie is just a markdown file that will be rendered as a HTML file. A overview of all snippies
will be generated as well.  
Put all of your code snippets as separate .md files into a directory (no subdirectories!) and run the tool.  
The generated output folder will contain all the necessary assets to statically serve the directory as-is.  
Under the hood snippies uses prism.js for syntax highlighting.

## Usage  

```
snippies [OPTIONS] --snippie-dir <SNIPPIE_DIR>

Options:
  -s, --snippie-dir <SNIPPIE_DIR>  Directory where snippie .md files reside
  -o, --out-dir <OUT_DIR>          Output directory, ready to serve
  -c, --clear-output-dir           Delete output directory contents before writing new files
  -h, --help                       Print help
```


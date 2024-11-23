# Glob Tree

A tree structure for matching strings against many glob patterns.

Each node in the tree represents a single character of a pattern.

**Example**
Take the pattern 'fo*', if inserted into an empty tree the tree would look like this:
```txt
root
  |_f
    |_o
      |_*
```

Many such patterns can be inserted into the tree. Use the `check()` method on a string to determine if any of the
patterns in the tree match that string.

The asterisk "*" is a wildcard character, matching any characters. For example, the pattern `foo*` matches both
`"food"` and `"football"`. A wildcard isn't limited to the end of the string,

The best applications of this data structure involve matching a high volume of strings against a large collection of distinct
patterns.
- Pub sub: Filtering messages sent to a client by checking the message topic against a tree of subscription patterns
- File system scanning: searching over a file system for files matching a set of patterns

Each node in the tree stores:
- a token (char)
- a reference count, indicating the number of distinct patterns that include that same node
- a collection of child nodes
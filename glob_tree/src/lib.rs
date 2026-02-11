//! A tree structure for matching strings against many glob patterns
//!
//! A directional tree structure for storing a collection of glob patterns and efficiently checking a new string against stored patterns.
//!
//! Each node in the tree represents a single character of a pattern.
//!
//! ## Example
//!
//! Take the pattern 'fo*', if inserted into an empty tree the tree would look like this:
//! ```txt
//! root
//!   |_f
//!     |_o
//!       |_*
//! ```
//!
//! Many such patterns can be inserted into the tree. Use the `check()` method on a string to determine if any of the
//! patterns in the tree match that string.
//!
//! The asterisk "*" is a wildcard character, matching any characters any number of times. For example, the pattern
//! `foo*` matches both `"food"` and `"football"`. A wildcard isn't limited to the end of the string, for example the
//! pattern `fo*l` would match the strings `"fool"` and `"foil"`, but not `"focus"`.
//!
//! The question mark "?" a wildcard character that can be used exactly once. For example, the pattern `foo?` would
//! match `"food"` but not `"football"`.
//!
//! The best applications of this data structure involve matching a high volume of strings against a large collection of distinct
//! patterns.
//! - Pub sub: Filtering messages sent to a client by checking the message topic against a tree of subscription patterns
//! - File system scanning: searching over a file system for files matching a set of patterns
//!
//! ```
//! use glob_tree::GlobTree;
//!
//! let mut tree = GlobTree::new();
//! tree.insert("foo*");
//! assert!(tree.check("food"));
//! ```

use std::clone::Clone;
use std::collections::BTreeMap;

const MULTI_CHARACTER_WILDCARD: char = '*';
const SINGLE_CHARACTER_WILDCARD: char = '?';

#[derive(Debug, Clone, PartialEq, Default)]
struct Node {
    // Token to store in this node
    token: Option<char>,
    /// Collection of child nodes, with their tokens as keys
    children: BTreeMap<char, Node>,
    /// Count of patterns which include this node
    count: u64,
}

impl Node {
    fn new(token: Option<char>) -> Self {
        Node {
            token,
            children: BTreeMap::new(),
            count: 1,
        }
    }
    /// Insert a child node
    fn insert_child(&mut self, child: Node) {
        if let Some(c) = child.token {
            self.children.insert(c, child);
        }
    }

    /// Increment reference count
    fn increment_count(&mut self) {
        self.count += 1;
    }

    /// Decrement reference count
    fn decrement_count(&mut self) {
        self.count -= 1;
    }

    /// The cumulative number of references to children of this node
    fn child_count(&self) -> u64 {
        self.children.iter().map(|(_, node)| node.count).sum()
    }

    /// Returns true if this node terminates a pattern
    fn is_terminal(&self) -> bool {
        self.child_count() < self.count
    }

    /// List all strings below this node
    ///
    /// Recursively appends child characters
    fn list_strings(&self) -> Vec<String> {
        let mut strings = vec![];
        let mut buffer = String::new();
        self._list_strings_worker(&mut strings, &mut buffer);
        strings
    }

    // Recursive target for list_strings() method
    fn _list_strings_worker(&self, strings: &mut Vec<String>, buffer: &mut String) {
        // Append this node's token (don't do anything if root node)
        if let Some(token) = self.token {
            buffer.push(token);
        }

        // If this node terminates a string, save current buffer to output
        if self.is_terminal() {
            strings.push(buffer.clone());
        }

        // Get strings of all child nodes
        for child in self.children.values() {
            child._list_strings_worker(strings, buffer);
        }

        // Pop the character we added
        if self.token.is_some() {
            buffer.pop();
        }
    }

    /// Search tree starting from this node for pattern matching the given string
    fn match_string(&self, s: &str) -> bool {
        self._match_string_worker(s.as_bytes(), 0)
    }

    fn _match_string_worker(&self, s: &[u8], pos: usize) -> bool {
        // Skip this if token is None (root node)
        if let Some(t) = self.token {
            if pos >= s.len() {
                // End of string but pattern continues
                return false;
            }

            let c = s[pos] as char;
            // if the next character in pattern doesn't match the current character in string or a wild card, exit
            if ![c, MULTI_CHARACTER_WILDCARD, SINGLE_CHARACTER_WILDCARD].contains(&t) {
                // Doesn't match
                return false;
            }
        }

        let next_pos = if self.token.is_some() { pos + 1 } else { pos };

        if next_pos < s.len() {
            let next_char = s[next_pos] as char;
            // Check for direct child
            if let Some(node) = self.children.get(&next_char) {
                if node._match_string_worker(s, next_pos) {
                    return true;
                }
            }

            // No match on next char in string, check single-char wildcard
            if let Some(node) = self.children.get(&SINGLE_CHARACTER_WILDCARD) {
                if node._match_string_worker(s, next_pos) {
                    return true;
                }
            }

            // No match on single-char wildcard, check for active multi-char wildcard.
            // skips appending the token to the pattern, and instead re-invokes using the new string but same pattern
            if let Some(t) = self.token {
                if t == MULTI_CHARACTER_WILDCARD {
                    if self._match_string_worker(s, next_pos) {
                        return true;
                    }
                }
            }

            // No match on active multi-char wildcard, check for new multi-char wildcard
            if let Some(node) = self.children.get(&MULTI_CHARACTER_WILDCARD) {
                if node._match_string_worker(s, next_pos) {
                    return true;
                }
            }
            // Doesn't match
            false
        } else {
            // End of string, check if pattern is terminal
            self.is_terminal()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GlobTree {
    root: Node,
}
impl GlobTree {
    /// Create a new, empty tree
    pub fn new() -> GlobTree {
        GlobTree {
            root: Node {
                token: None,
                children: BTreeMap::new(),
                count: 0,
            },
        }
    }

    /// Add a pattern to the tree
    pub fn insert(&mut self, pattern: &str) {
        // Cursor stores the current node
        let mut cursor = &mut self.root;
        for c in pattern.chars() {
            match cursor.children.get(&c) {
                Some(_) => {
                    cursor = cursor.children.get_mut(&c).expect("Existing node exists");
                    cursor.increment_count();
                }
                None => {
                    cursor.insert_child(Node::new(Some(c)));
                    cursor = cursor.children.get_mut(&c).expect("Inserted node exists");
                }
            }
        }
        // Increment count on final node to mark pattern as terminal
        // For empty patterns, this increments root; for others, the last character node
        cursor.increment_count();
    }

    /// Check if a string is matched by a pattern in this tree
    pub fn check(&self, s: &str) -> bool {
        self.root.match_string(s)
    }

    /// Returns true if tree contains pattern
    pub fn contains(&self, pattern: &str) -> bool {
        let mut cursor = &self.root;
        for c in pattern.chars() {
            let child = cursor.children.get(&c);
            if child.is_none() {
                return false;
            }
            cursor = child.unwrap()
        }
        // Reached end of pattern, but tree may continue if there are overlapping patterns.
        // If the reference count of this node's children is less than the reference count of this node
        // then we know the tree contains a pattern that terminates on this node
        cursor.is_terminal()
    }

    /// List all patterns in the tree
    pub fn list(&self) -> Vec<String> {
        self.root.list_strings()
    }

    /// Remove a pattern from the tree
    pub fn remove(&mut self, pattern: &str) -> Result<(), &str> {
        if !self.contains(pattern) {
            return Err("Not found");
        }
        // Delete subscription from Tree
        let mut cursor = &mut self.root;
        for c in pattern.chars() {
            let child = cursor.children.get(&c).unwrap();
            if child.count == 1 {
                // Only one reference to this token, can delete node and all children
                cursor.children.remove(&c);
                return Ok(());
            } else {
                // This token is referenced elsewhere, decrement count and do nothing
                cursor.children.get_mut(&c).unwrap().decrement_count();
                cursor = cursor.children.get_mut(&c).unwrap();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check() {
        let mut t = GlobTree::new();
        assert!(!t.check("metrics"));
        t.insert("metrics");
        assert!(t.check("metrics"));
    }

    #[test]
    fn test_remove() {
        let mut t = GlobTree::new();
        t.insert("metrics");
        assert!(t.check("metrics"));
        t.remove("metrics").unwrap();
        assert!(t.root.children.is_empty());
        assert!(!t.check("metrics"));
    }

    #[test]
    fn test_wildcard_simple() {
        let mut t = GlobTree::new();
        t.insert(&MULTI_CHARACTER_WILDCARD.to_string());
        assert!(t.check("foo"));
    }

    #[test]
    fn test_inner_wildcard() {
        let mut t = GlobTree::new();
        let pattern = format!("fooo{}ar", MULTI_CHARACTER_WILDCARD);
        t.insert(&pattern);
        assert!(t.check("fooozar"));
        assert!(t.check("fooo/zbasfjadfasldfjah/ar"));
        assert!(!t.check("fooo/zbasfjadfasldfjah/"));
        assert!(!t.check("fooozarnt"));
    }

    #[test]
    fn test_partial_match() {
        let mut t = GlobTree::new();
        t.insert("testing");
        assert!(!t.check("test"));
    }

    #[test]
    fn test_check_with_overlapping() {
        let mut t = GlobTree::new();
        t.insert("testing");
        t.insert("test");
        assert!(t.check("test"));
    }

    #[test]
    fn test_contains() {
        let mut t = GlobTree::new();
        t.insert("testing");
        assert!(t.contains("testing"));
    }

    #[test]
    fn test_doesnt_contain_substring() {
        let mut t = GlobTree::new();
        t.insert("testing");
        assert!(!t.contains("test"));
    }

    #[test]
    fn node_list_strings() {
        let mut a = Node::new(Some('a'));
        let mut b = Node::new(Some('b'));
        let mut c = Node::new(Some('c'));
        let b2 = Node::new(Some('b'));
        let d = Node::new(Some('d'));
        // c included 2 times
        c.insert_child(d);
        c.increment_count();

        // b included three times (the node, not the character 'b')
        b.insert_child(c);
        b.insert_child(b2);
        b.increment_count();
        b.increment_count();

        // a included three times
        a.insert_child(b);
        a.increment_count();
        a.increment_count();
        assert_eq!(a.list_strings(), vec!["abb", "abc", "abcd"]);
    }

    #[test]
    fn test_list() {
        let mut t = GlobTree::new();
        let mut patterns = vec![
            ".venv",
            ".DS_STORE",
            "~/Documents/Important/2024/*",
            "*.exe",
        ];
        for pattern in &patterns {
            t.insert(&pattern);
        }
        assert_eq!(patterns.sort(), t.list().sort());
    }

    #[test]
    fn test_node_child_count() {
        let mut node = Node::new(Some('a'));
        assert_eq!(node.child_count(), 0);
        node.insert_child(Node::new(Some('b')));
        assert_eq!(node.child_count(), 1);
    }

    #[test]
    fn test_fixed_length_wildcard_simple() {
        let mut t = GlobTree::new();
        t.insert("ab?d");
        assert!(t.check("abcd"));
        assert!(!t.check("abccd"));
    }

    #[test]
    fn test_fixed_length_wildcard_overlapping_variable_length_wildcard() {
        let mut t = GlobTree::new();
        t.insert("ab?d");
        t.insert("ab*");
        assert!(t.check("abcd"));
        assert!(t.check("abccd"));
    }

    #[test]
    fn test_empty_string_matches_empty_pattern() {
        let mut t = GlobTree::new();
        t.insert("");
        assert!(t.check(""));
    }
}

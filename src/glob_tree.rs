/// Glob tree
///
/// A directional tree structure for storing a collection of glob patterns and efficiently checking a new string against stored patterns.
///
/// Each node in the tree represents a single character of a pattern.
///
/// **Example**
/// Take the pattern 'fo*', if inserted into an empty tree the tree would look like this:
/// ```txt
/// root
///   |_f
///     |_o
///       |_*
/// ```
///
/// Many such patterns can be inserted into the tree. Use the `check()` method on a string to determine if any of the
/// patterns in the tree match that string.
///
/// The asterisk "*" is a wildcard character, matching any characters. For example, the pattern `foo*` matches both
/// `"food"` and `"football"`. A wildcard isn't limited to the end of the string,
///
/// The best applications of this data structure involve matching a high volume of strings against a large collection of distinct
/// patterns.
/// - Pub sub: Filtering messages sent to a client by checking the message topic against a tree of subscription patterns
/// - File system scanning: searching over a file system for files matching a set of patterns
///
/// Each node in the tree stores:
/// - a token (char)
/// - a reference count, indicating the number of distinct patterns that include that same node
/// - a collection of child nodes
use std::clone::Clone;
use std::collections::BTreeMap;

use crate::config;

#[derive(Debug, Clone, PartialEq)]
struct Node {
    // Token to store in this node
    token: Option<char>,
    /// Collection of child nodes, with their tokens as keys
    children: BTreeMap<char, Node>,
    /// Count of patterns which include this node
    count: u64,
}
impl Node {
    fn new(token: Option<char>) -> Node {
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
        self._list_strings_worker("".into())
    }

    // Recursive target for list_strings() method
    fn _list_strings_worker(&self, mut string: String) -> Vec<String> {
        // Create output vector
        let mut strings = vec![];

        // Append this node's token token (don't do anything if root node)
        if let Some(token) = self.token {
            string.push(token);
        }
        // If this node terminates a string, append string to the output
        if self.is_terminal() {
            strings.push(string.clone())
        }

        // Get strings of all child nodes, and append to output
        for child in self.children.values() {
            let mut child_strings = child._list_strings_worker(string.clone());
            strings.append(&mut child_strings);
        }

        strings
    }
}

#[derive(Debug)]
pub struct GlobTree {
    root: Node,
}
impl GlobTree {
    /// Create a new, empty tree
    pub fn new() -> GlobTree {
        GlobTree {
            root: Node::new(None),
        }
    }

    /// Add a pattern to the tree
    ///
    /// Args:
    ///     pattern: a pattern to insert
    ///
    ///
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
    }

    /// Check if a string is matched by a pattern in this tree
    ///
    /// Args
    /// - s: the string to match
    pub fn check(&self, s: &str) -> bool {
        let mut cursor = &self.root;
        let mut active_wildcard: Option<&Node> = None;
        for c in s.chars() {
            // Get next character from chldren
            let mut next = cursor.children.get(&c);

            // If not found, look for wildcard
            if next.is_none() {
                next = cursor.children.get(&config::WILDCARD);
                if next.is_some() {
                    active_wildcard = next;
                }
            }

            // If next
            match next {
                // Next character is in tree, continue check
                Some(node) => {
                    cursor = node;
                }
                // No children matching next token in pattern
                None => {
                    match active_wildcard {
                        // There is an active wildcard, reset cursor to this node and continue searching
                        Some(node) => {
                            // Last character in pattern is a wildcard, string is matched
                            if node.is_terminal() {
                                return true;
                            }
                            // There are more tokens in pattern beyond wildcard, so reset cursor to wildcard node.
                            // This is necessary when a wildcard is followed by several characters (e.g. '*1234').
                            // All characters beyond the wildcard must be matched consecutively for the string to be a
                            // match. E.g. `1235_1234` matches `*1234`, but not until the end. When we encounter the
                            // token '5', this doesn't invalidate the match (because of the wildcard), but we also need
                            // to  reset the active token in the cursor to the wildcard to correctly match the next
                            // token in the pattern (in this case, '1').
                            cursor = node
                        }
                        // No active wildcard, string doesn't match any patterns
                        None => return false,
                    }
                }
            }
        }
        // Reached end of s, but tree may continue if there are overlapping patterns.
        // If the reference count of this node's children is less than the reference count of this node
        // then we know the tree contains a pattern that terminates on this node
        cursor.is_terminal()
    }

    /// Returns true if tree contains pattern
    fn contains(&mut self, pattern: &str) -> bool {
        let mut cursor = &mut self.root;
        for c in pattern.chars() {
            let child = cursor.children.get_mut(&c);
            if child.is_none() {
                return false;
            }
            cursor = child.unwrap()
        }
        // Reached end of pattern, but tree may continue if there are overlapping patterns.
        // If the reference count of this node's children is less than the reference count of this node
        // then we know the tree contains a pattern that terminates on this node
        return cursor.is_terminal();
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
            let child = cursor.children.get_mut(&c).unwrap();
            if child.count == 1 {
                // Only one reference to this token, can delete node and all children
                cursor.children.remove(&c);
                return Ok(());
            } else {
                // This token is referenced elsewhere, decrement_count count and do nothing
                child.decrement_count();
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
    fn test_wildcard() {
        let mut t = GlobTree::new();
        t.insert(&config::WILDCARD.to_string());
        assert!(t.check("foo"));
    }

    #[test]
    fn test_inner_wildcard() {
        let mut t = GlobTree::new();
        t.insert(&format!("fooo{}ar", config::WILDCARD));
        assert!(t.check("fooozar".into()));
        assert!(t.check("fooo/zbasfjadfasldfjah/ar".into()));
        assert!(!t.check("fooo/zbasfjadfasldfjah/".into()));
        assert!(!t.check("fooozarnt".into()));
    }

    #[test]
    fn test_partial_match() {
        let mut t = GlobTree::new();
        t.insert("testing");
        assert!(!t.check("test".into()));
    }

    #[test]
    fn test_check_with_overlapping() {
        let mut t = GlobTree::new();
        t.insert("testing");
        t.insert("test");
        assert!(t.check("test".into()));
    }

    #[test]
    fn test_contains() {
        let mut t = GlobTree::new();
        t.insert("testing");
        assert!(t.contains("testing".into()));
    }

    #[test]
    fn test_doesnt_contain_substring() {
        let mut t = GlobTree::new();
        t.insert("testing");
        assert!(!t.contains("test".into()));
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
}

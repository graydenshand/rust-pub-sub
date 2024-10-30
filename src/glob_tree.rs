/// Glob tree
///
/// A directional tree structure for storing a collection of glob patterns and efficiently checking a new string against stored patterns.
///
/// Each node in the tree represents a character of a pattern.
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
/// The asterisk "*" is a wildcard character, matching any character.
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
use std::collections::HashMap;

use crate::config;

#[derive(Debug, Clone, PartialEq)]
struct Node {
    // Token to store in this node
    token: Option<char>,
    /// Collection of child nodes, with their tokens as keys
    children: HashMap<char, Node>,
    /// Count of patterns which include this node
    count: u64,
}
impl Node {
    fn new(token: Option<char>) -> Node {
        Node {
            token,
            children: HashMap::new(),
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
        for c in s.chars() {
            // Get next character from chldren
            let mut next = cursor.children.get(&c);

            // If not found, look for wildcard
            if next.is_none() {
                next = cursor.children.get(&config::WILDCARD);
            }

            // If next
            match next {
                // Next character is in tree, continue check
                Some(node) => {
                    cursor = node;
                }
                // Terminal node (no children), return false if previous character was not a wildcard
                None => match cursor.token {
                    Some(token) => return token == config::WILDCARD,
                    // root node has no children, empty tree
                    None => return false,
                },
            }
        }
        // Reached end of s, but tree may continue if there are overlapping patterns.
        // If the reference count of this node's children is less than the reference count of this node
        // then we know the tree contains a pattern that terminates on this node
        cursor.child_count() < cursor.count
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
        return cursor.child_count() < cursor.count;
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
}

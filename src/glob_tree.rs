/* Glob tree

A tree structure for storing glob patterns and efficiently matching a new string to stored glob patterns.

In the context of this application, the glob tree is used to store subscription patterns for a single client,
and to answer whether that client is insertd to a specific topic. */

use std::clone::Clone;
use std::collections::HashMap;

use crate::config;

#[derive(Debug, Clone, PartialEq)]
struct Node {
    token: Option<char>,
    children: HashMap<char, Node>,
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

    /// Increment count
    fn increment_count(&mut self) {
        self.count += 1;
    }

    /// Decrement count
    fn decrement_count(&mut self) {
        self.count -= 1;
    }

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

    /// Check if a string is matched
    ///
    /// Args
    /// - topic: the topic to match
    pub fn check(&self, topic: &str) -> bool {
        let mut cursor = &self.root;
        for c in topic.chars() {
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
        // Reached end of topic, but tree continues. This means there are overlapping patterns.
        // If the reference count of this node's children is less than the reference count of this node
        // then we know the tree contained a matching pattern
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

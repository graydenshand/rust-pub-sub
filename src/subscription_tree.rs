/* Glob tree

A tree structure for storing glob patterns and efficiently matching a new string to stored glob patterns.

In the context of this application, the glob tree is used to store subscription patterns for a single client, 
and to answer whether that client is subscribed to a specific topic. */

use std::clone::Clone;
use std::collections::HashMap;

use crate::config;

#[derive(Debug, Clone, PartialEq)]
struct Node{
    token: Option<char>,
    children: HashMap<char, Node>,
    count: u64,
}
impl Node {
    fn new(token: Option<char>) -> Node<> {
        Node {
            token,
            children: HashMap::new(),
            count: 1
        }
    }

    fn insert_child(&mut self, child: Node) {
        if let Some(c) = child.token {
            self.children.insert(c, child);
        }
    }

    fn increment(&mut self) {
        self.count += 1;
    }
    
    fn decrement(&mut self) {
        self.count -= 1;
    }
}

#[derive(Debug)]
pub struct SubscriptionTree {
    root: Node,
}
impl SubscriptionTree {
    /// Create a new, empty tree
    pub fn new() -> SubscriptionTree {
        SubscriptionTree {
            root: Node::new(None),
        }
    }

    /// Add a subscription to the tree
    pub fn subscribe(&mut self, pattern: &str) {
        // Cursor stores the current node
        let mut cursor = &mut self.root;
        for c in pattern.chars() {
            match cursor.children.get(&c) {
                Some(node) => {
                    cursor = cursor.children.get_mut(&c).expect("Existing node exists");
                    cursor.increment();
                },
                None => {
                    cursor.insert_child(Node::new(Some(c)));
                    cursor = cursor.children.get_mut(&c).expect("Inserted node exists");
                }
            }
        }
    }

    /// Return the full set of subscribers to a given topic
    ///
    /// Args
    /// - topic: the topic to match
    pub fn is_subscribed(&self, topic: &str) -> bool {
        let mut cursor = &self.root;
        for c in topic.chars() {
            // Skip this check if first character of topic is the system_topic_prefix
            if !(cursor == &self.root && c == config::SYSTEM_TOPIC_PREFIX) {
                // If wildcard character in children, topic is matched -- return True
                if  cursor.children.get(&config::WILDCARD).is_some() { return true };
            }

            // println!("{:?} {:?}", c, cursor.children);

            match cursor.children.get(&c) {
                // Next character is in tree, continue search
                Some(node) => {
                    cursor = node;
                },
                // Next character is not in tree, topic is not matched -- return False
                None => {return false}
            }

        }
        // A pattern fully matches this topic, return True
        true 
    }

    /// Returns true if pattern is in tree
    fn pattern_in_tree(&mut self, pattern: &str) -> bool {
        let mut cursor = &mut self.root;
        for c in pattern.chars() {
            let child = cursor.children.get_mut(&c);
            if child.is_none() {
                return false
            }
            cursor = child.unwrap()
        }
        return true
    }

    /// Delete a subscription
    pub fn unsubscribe(&mut self, pattern: &str) -> Result<(), &str> {
        if !self.pattern_in_tree(pattern) {
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
                // This token is referenced elsewhere, decrement count and do nothing
                child.decrement();
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
    fn test_subscribe() {
        let mut st = SubscriptionTree::new();
        assert!(!st.is_subscribed("metrics"));
        st.subscribe("metrics");
        assert!(st.is_subscribed("metrics"));
    }

    #[test]
    fn test_unsubscribe() {
        let mut st = SubscriptionTree::new();
        st.subscribe("metrics");
        assert!(st.is_subscribed("metrics"));
        st.unsubscribe("metrics").unwrap();
        assert!(st.root.children.is_empty());
        assert!(!st.is_subscribed("metrics"));
    }

    #[test]
    fn test_wildcard() {
        let mut st = SubscriptionTree::new();
        st.subscribe(&format!("{}", config::WILDCARD));
        assert!(st.is_subscribed("foo"));
    }

    #[test]
    fn test_system_topics_filter() {
        let mut st = SubscriptionTree::new();
        st.subscribe(&format!("{}", config::WILDCARD));
        assert!(!st.is_subscribed(&format!("{}foo", config::SYSTEM_TOPIC_PREFIX)));
    }
}

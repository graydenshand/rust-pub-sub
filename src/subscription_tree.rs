use std::clone::Clone;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::hash::Hash;
use std::iter::Peekable;
use std::str::Chars;
use std::str::FromStr;

#[derive(Debug, Clone)]
struct Node<T: Clone + Eq + Hash> {
    token: Option<char>,
    children: HashMap<char, Node<T>>,
    items: Vec<T>,
}
impl<T> Node<T>
where
    T: Clone + Eq + Hash,
{
    fn new(token: Option<char>) -> Node<T> {
        Node {
            token,
            children: HashMap::new(),
            items: vec![],
        }
    }

    fn insert_child(&mut self, child: Node<T>) {
        if let Some(c) = child.token {
            self.children.insert(c, child);
        }
    }
}

#[derive(Debug)]
pub struct SubscriptionTree<T: Clone + Eq + Hash> {
    /// The root node in the subscription tree
    root: Node<T>,
    /// Maping of client to a set of its subscription patterns
    subscribers: HashMap<T, HashSet<String>>,
}
impl<T> SubscriptionTree<T>
where
    T: Clone + Eq + Hash,
{
    /// Create a new, empty tree
    pub fn new() -> SubscriptionTree<T> {
        SubscriptionTree {
            root: Node::new(None),
            subscribers: HashMap::new(),
        }
    }

    /// Add a subscription to the tree
    pub fn subscribe(&mut self, pattern: &str, id: T) {
        // Add pattern to subscriber's map
        if let Some(existing_subscribers) = self.subscribers.get_mut(&id) {
            existing_subscribers.insert(pattern.to_string());
        } else {
            self.subscribers
                .insert(id.clone(), HashSet::from([pattern.to_string()]));
        }

        // Add client to pattern tree
        let mut cursor = &mut self.root;
        let mut exists: bool;
        for c in pattern.chars() {
            // This weird pattern was used to get around a borrow checker error trying
            // to mutable borrow the cursor node twice at the same time inside a match statement
            if let Some(_) = cursor.children.get(&c) {
                exists = true;
            } else {
                exists = false;
            }

            if exists {
                cursor = cursor.children.get_mut(&c).expect("Existing node exists")
            } else {
                cursor.insert_child(Node::new(Some(c)));
                let new = cursor.children.get_mut(&c).expect("Inserted node exists");
                cursor = new;
            }
        }
        cursor.items.push(id);
    }

    /// Private, recursive function for getting all subscribers matching chars below start node
    fn collect_subscribers(
        mut chars: Peekable<Chars>,
        subscribers: &mut HashSet<T>,
        node: &Node<T>,
    ) {
        let next = chars.next();

        let wildcard = node.children.get(&char::from_str("*").unwrap());
        if let Some(wildcard_node) = wildcard {
            for item in wildcard_node.items.clone() {
                subscribers.insert(item);
            }
            SubscriptionTree::collect_subscribers(chars.clone(), subscribers, wildcard_node);
        }

        let next_node_opt = node.children.get(&next.unwrap());
        match next_node_opt {
            Some(next_node) => {
                if chars.peek().is_none() {
                    // iterator is exhausted, full match
                    for item in next_node.items.clone() {
                        subscribers.insert(item);
                    }
                    return;
                }
                SubscriptionTree::collect_subscribers(chars, subscribers, next_node)
            }
            // Next character not in tree, no more subscriptions on this path
            None => return,
        }
    }

    /// Return the full set of subscribers to a given topic
    pub fn get_subscribers(&self, topic: &str) -> HashSet<T> {
        let mut subscribers = HashSet::new();
        SubscriptionTree::collect_subscribers(
            topic.chars().peekable(),
            &mut subscribers,
            &self.root,
        );
        subscribers
    }

    /// Delete a subscription
    pub fn unsubscribe(&mut self, id: &T, pattern: &str) -> Result<(), &str> {
        if let Some(patterns) = self.subscribers.get_mut(&id) {
            // Delete pattern from subscribers map
            patterns.remove(pattern);
            if patterns.is_empty() {
                self.subscribers.remove(&id);
            }

            // Delete subscription from Tree
            // Traverse the tree to find leaf node
            let mut cursor = &mut self.root;
            let mut path = vec![cursor.clone()];
            for c in pattern.chars() {
                if let Some(node) = cursor.children.get_mut(&c) {
                    cursor = node;
                    // Clone may be inefficient, but Nodes are small and this is much simpler than alternatives
                    path.push(cursor.clone());
                } else {
                    return Err("Not found");
                }
            }
            // Terminal node found for this subscription, delete item from node
            let idx = cursor.items.iter().position(|i| i == id).unwrap();
            cursor.items.remove(idx);

            // Walk back up the path, deleting any empty nodes (no items or children)
            while path.len() > 1 {
                let node = path.pop().unwrap();
                if node.items.is_empty() && node.children.is_empty() {
                    let parent = path.last_mut().unwrap();
                    parent.children.remove(&node.token.unwrap());
                } else {
                    break;
                }
            }

            Ok(())
        } else {
            Err("Not found")
        }
    }

    pub fn unsubscribe_client(&mut self, id: T) -> Result<(), &str> {
        match self.subscribers.get(&id) {
            Some(client_subscriptions) => {
                let patterns = client_subscriptions.clone();
                for pattern in patterns {
                    self.unsubscribe(&id, &pattern)
                        .ok()
                        .expect("Unsubscribe was successful");
                }
                self.subscribers.remove(&id);
                Ok(())
            }
            None => Err("Not found"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe() {
        let mut st = SubscriptionTree::new();
        st.subscribe("metrics", &0);
        st.subscribe("*", &1);
        st.subscribe("met*ic", &2);
        let key = char::from_str("m").unwrap();
        st.root.children.get(&key).expect("exists");
        assert!(st.get_subscribers("test") == HashSet::from([&1]));
        assert!(st.get_subscribers("metrics") == HashSet::from([&0, &1]));

        st.unsubscribe(&&0, "metrics").unwrap();
        assert!(st.get_subscribers("metrics") == HashSet::from([&1]));

        st.unsubscribe_client(&1).unwrap();
        assert!(st.get_subscribers("metrics") == HashSet::new());
    }
}

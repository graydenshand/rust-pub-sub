use std::clone::Clone;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::iter::Peekable;
use std::str::Chars;
use std::str::FromStr;

#[derive(Debug)]
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

struct SubscriptionTree<T: Clone + Eq + Hash> {
    root: Node<T>,
}
impl<T> SubscriptionTree<T>
where
    T: Clone + Eq + Hash,
{
    /// Create a new, empty tree
    fn new() -> SubscriptionTree<T> {
        SubscriptionTree {
            root: Node::new(None),
        }
    }

    /// Add a subscription to the tree
    fn subscribe(&mut self, pattern: &str, id: T) {
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

    fn get_subscribers(&self, topic: &str) -> HashSet<T> {
        let mut subscribers = HashSet::new();
        SubscriptionTree::collect_subscribers(
            topic.chars().peekable(),
            &mut subscribers,
            &self.root,
        );
        subscribers
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
    }
}

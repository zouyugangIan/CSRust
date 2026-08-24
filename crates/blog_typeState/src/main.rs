use blog_state::*;

fn main() {
    let mut post = Post::new();

    post.add_text("This morning, I got up early and learned rust for a while!");
    assert_eq!("", post.content());

    post.request_review();
    assert_eq!("", post.content());

    post.approve();
    assert_eq!(
        "This morning, I got up early and learned rust for a while!",
        post.content()
    );
    println!("Content:{}", post.content());
}

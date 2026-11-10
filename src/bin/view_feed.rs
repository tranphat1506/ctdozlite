//! Binary CLI hiển thị dữ liệu dòng thời gian thực tế 100% của bạn bè (Đtc)
//!
//! Chạy với: `cargo run --bin view_feed`

use anyhow::Result;
use std::path::Path;
use zalo_rust::feed::load_real_timeline_from_sqlite;

fn main() -> Result<()> {
    println!("======================================================================");
    println!("📱 ZALO-RUST REAL SOCIAL TIMELINE VIEWER (100% REAL DATA)");
    println!("======================================================================\n");

    let db_candidates = [
        "test_lab/databases/zalo",
        "zalo-rust/test_lab/databases/zalo",
        "databases/zalo",
    ];

    let mut db_path_found = None;
    for c in db_candidates {
        if Path::new(c).exists() {
            db_path_found = Some(c);
            break;
        }
    }

    let db_path = match db_path_found {
        Some(p) => p,
        None => {
            eprintln!("❌ Không tìm thấy database tại test_lab/databases/zalo");
            return Ok(());
        }
    };

    println!("✓ Đã kết nối cơ sở dữ liệu SQLite: {db_path}");
    println!("✓ Nạp toàn bộ dòng thời gian Nhật ký (Tất cả bạn bè)\n");

    let feeds = load_real_timeline_from_sqlite(db_path, "")?;
    println!("🎯 Tìm thấy {} bài đăng thực tế:\n", feeds.len());

    for (idx, item) in feeds.iter().enumerate() {
        println!("----------------------------------------------------------------------");
        println!("📌 BÀI ĐĂNG #{} [Feed ID: {}]", idx + 1, item.feed_id);
        println!(
            "👤 Tác giả:   {} (UID: {})",
            item.author.display_name, item.author.uid
        );
        println!("🕒 Thời gian: {}", item.date_str);
        println!(
            "❤️ Tương tác: {} lượt thích | 💬 {} bình luận",
            item.reaction.like_count, item.reaction.comment_count
        );
        for m in &item.media_list {
            println!("🖼️ Ảnh CDN:   {} ({}x{} px)", m.url, m.width, m.height);
        }
        println!("\n📝 Nội dung bài viết:\n{}\n", item.content.trim());
    }

    println!("======================================================================");
    println!("✓ Hoàn tất đọc dữ liệu thật từ SQLite Zalo.");
    println!("======================================================================");

    Ok(())
}

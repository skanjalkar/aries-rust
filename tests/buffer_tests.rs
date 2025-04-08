use aries_rust::{
    buffer::{BufferManager, BufferFrame},
    common::{PageID, Result},
};

#[test]
fn test_buffer_manager_basic() -> Result<()> {
    let mut buffer_manager = BufferManager::new(4096, 2); // Capacity of 2 pages
    
    // Fix two pages
    let page1 = PageID(1);
    let page2 = PageID(2);
    
    let frame1 = buffer_manager.fix_page(page1, false)?;
    let frame2 = buffer_manager.fix_page(page2, false)?;
    
    // Try to fix a third page - should fail because buffer is full
    let page3 = PageID(3);
    assert!(buffer_manager.fix_page(page3, false).is_err());
    
    // Unfix one page
    buffer_manager.unfix_page(frame1, false)?;
    
    // Now should be able to fix another page
    let frame3 = buffer_manager.fix_page(page3, false)?;
    
    Ok(())
}

#[test]
fn test_buffer_manager_dirty_pages() -> Result<()> {
    let mut buffer_manager = BufferManager::new(4096, 2);
    
    // Fix a page in exclusive mode
    let page1 = PageID(1);
    let frame1 = buffer_manager.fix_page(page1, true)?;
    
    // Modify the page
    {
        let mut frame = frame1.lock().unwrap();
        let data = frame.get_data_mut();
        data[0] = 42;
        frame.set_dirty(true);
    }
    
    // Unfix the page
    buffer_manager.unfix_page(frame1, true)?;
    
    // Flush the page
    buffer_manager.flush_page(page1)?;
    
    Ok(())
}
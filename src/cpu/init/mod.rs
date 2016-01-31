pub mod gdt;
pub mod idt;
pub mod tss;

extern "C" {
    fn _bp_handler();
    fn _gp_handler();
}

/// Unsafe because dropping gdt or idt leaks a reference
pub unsafe fn setup() -> (gdt::Table, idt::Table) {
    // create a new GDT with a TSS
    let tss = tss::Segment::new([None, None, None, None, None, None, None],
                                [None, None, None], 0);

    let mut gdt = gdt::Table::new(vec![tss]);

    debug!("Created new GDT");

    // install the gdt
    gdt.install();

    debug!("Installed GDT");

    // set the task
    gdt.set_task(0);

    debug!("Set new task");

    let mut descriptors = vec![];

    descriptors.push(idt::Descriptor::placeholder()); // 0
    descriptors.push(idt::Descriptor::placeholder()); // 1
    descriptors.push(idt::Descriptor::placeholder()); // 2
    descriptors.push(idt::Descriptor::new(_bp_handler as u64, 0)); // 3
    descriptors.push(idt::Descriptor::placeholder()); // 4
    descriptors.push(idt::Descriptor::placeholder()); // 5
    descriptors.push(idt::Descriptor::placeholder()); // 6
    descriptors.push(idt::Descriptor::placeholder()); // 7
    descriptors.push(idt::Descriptor::placeholder()); // 8
    descriptors.push(idt::Descriptor::placeholder()); // 9
    descriptors.push(idt::Descriptor::placeholder()); // 10
    descriptors.push(idt::Descriptor::placeholder()); // 11
    descriptors.push(idt::Descriptor::placeholder()); // 13
    descriptors.push(idt::Descriptor::new(_gp_handler as u64, 0)); // 14
    descriptors.push(idt::Descriptor::placeholder()); // 15

    let mut idt = idt::Table::new(descriptors);

    debug!("Created IDT");

    idt.install();

    debug!("Installed IDT");

    (gdt, idt)
}

use include::*;

use log_abi::Location;

static MANAGER: Mutex<Option<Manager>> = Mutex::new(None);

pub trait Frame {
    fn add(&mut self, point: PointRef);
}

pub struct PointFrame {
    traces: Vec<PointRef>
}

#[derive(Debug)]
#[must_use]
pub struct PointRef {
    id: usize
}

#[derive(Debug)]
struct Point {
    id: usize,
    message: String
}

#[derive(Debug)]
struct Manager {
    traces: Vec<Point>,
    next_id: usize
}

impl Drop for PointRef {
    fn drop(&mut self) {
        if let Some(ref mut manager) = *MANAGER.lock() {
            manager.untrace(self.id);
        } else {
            unreachable!("Dropped a PointRef with no active Manager");
        }
    }
}

impl Drop for PointFrame {
    fn drop(&mut self) {
        // drop in reverse order
        while self.traces.pop().is_some() {}
    }
}

impl Display for Point {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.message)
    }
}

impl Display for Manager {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(fmt, "Trace points:"));

        for point in self.traces.iter() {
            try!(write!(fmt, "\n{}", point));
        }

        Ok(())
    }
}

impl Frame for PointFrame {
    #[inline]
    fn add(&mut self, point: PointRef) {
        self.traces.push(point);
    }
}

impl PointFrame {
    pub fn new() -> PointFrame {
        PointFrame {
            traces: vec![]
        }
    }

    pub fn new_add<T: Display>(location: &Location, message: T) -> PointFrame {
        PointFrame {
            traces: vec![trace(location, message)]
        }
    }
}

impl Manager {
    fn new() -> Manager {
        Manager {
            traces: vec![],
            next_id: 0
        }
    }

    fn trace<T: Display>(&mut self, location: &Location, message: T) -> PointRef {
        self.traces.push(Point {
            id: self.next_id,
            message: format!("{} at {}({}): {}", location.module_path, location.file, location.line, message)
        });

        let point = PointRef {
            id: self.next_id
        };

        self.next_id += 1;

        point
    }

    fn untrace(&mut self, id: usize) {
        debug!("Untracing id {}", id);

        loop {
            if let Some(trace) = self.traces.pop() {
                trace!("{:?}", trace);
                if trace.id == id {
                    trace!("Found matching");
                    break;
                }
            } else {
                panic!("untraced non-containing ID: {}", id);
            }
        }
    }
}

pub fn trace<T: Display>(location: &Location, message: T) -> PointRef {
    let mut outer = MANAGER.lock();

    if outer.is_none() {
        let mut manager = Manager::new();
        let result = manager.trace(location, message);
        *outer = Some(manager);
        result
    } else if let Some(ref mut manager) = *outer {
        manager.trace(location, message)
    } else {
        unreachable!();
    }
}

pub fn write_trace<T: Write>(into: &mut T) -> fmt::Result {
    if let Some(ref manager) = *MANAGER.lock() {
        write!(into, "{}", manager)
    } else {
        Err(fmt::Error)
    }
}

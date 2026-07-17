use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use tokio::{sync::mpsc};

// We implement custom logic for printing messages, 
// as they need to work properly with download progress bar


pub enum PrinterMessage {
    Text(String),
    SetupPB(usize),
    IncPB,
    FinalizePB,
}

struct Printer {
    rx: mpsc::Receiver<PrinterMessage>,
    pb: Option<ProgressBar>,
    active: bool,
}

pub type PrintTx = mpsc::Sender<PrinterMessage>;

pub fn create_printer() -> (impl Future<Output=()>, PrintTx) {
    const BUF_SIZE: usize = 10;
    let (tx, rx) = mpsc::channel(BUF_SIZE);
    let printer = Printer {rx, pb: None, active: true, };
    (printer.printer_future(), tx)
}

impl Printer {
    
    async fn printer_future(mut self) {
        while let Some(msg) = self.rx.recv().await {
            self.process(msg);
        }
    }

    fn process(&mut self, msg: PrinterMessage) {
        // Simply ignore progress bar-modifying messages if it's not set up yet.
        // Also, allow to replace progress bar, although we don't use it
        match msg {
            PrinterMessage::Text(msg) => self._println(msg),
            PrinterMessage::SetupPB(size) => {
                self.setup_pb(size);
                self.active = true;
            },
            PrinterMessage::IncPB =>  
                match &self.pb {
                    Some(pb) => {
                        if self.active { pb.inc(1); }
                    },
                    None => (),
                },
            PrinterMessage::FinalizePB =>
                match &self.pb {
                    Some(pb) => {
                        if self.active {
                            pb.tick(); // to force redraw the current state
                        };
                        self.active = false;
                    }
                    None => (),
                },
        }
    }

    fn setup_pb(&mut self, size: usize) {        
        let pb = ProgressBar::new(size as u64);
        const STYLE: &str = "{msg}:{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len}";
        pb.set_style(ProgressStyle::with_template(STYLE)
            .unwrap()
            .progress_chars("#>-"));
        pb.set_draw_target(ProgressDrawTarget::stdout());
        pb.set_message("Download progress");
        self.pb = Some(pb);
    }

    fn _println(&self, msg: String) {
        if !self.active { return };
        match &self.pb {
            Some(pb) => pb.println(msg),
            None => println!("{}", msg),
        }
    }
}

pub trait PrintMessageCallback: Future<Output = ()> + Send + 'static {}
impl<T: Future<Output = ()> + Send + 'static> PrintMessageCallback for T {}
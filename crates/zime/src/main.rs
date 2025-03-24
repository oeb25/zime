mod remotes;

use std::fs;

use biblatex::ChunksExt;
use camino::Utf8PathBuf;
use clap::Parser as _;
use color_eyre::{eyre::eyre, owo_colors::OwoColorize};
use config::Setup;
use duct::cmd;
use itertools::Itertools;
use remotes::arxiv::is_arxiv;
use tracing::{debug, info, warn};

type Result<T, E = color_eyre::eyre::Error> = std::result::Result<T, E>;

#[derive(Debug, clap::Parser)]
struct Cli {
    #[clap(subcommand)]
    cmd: Command,
    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    Init {
        #[clap(short, long)]
        /// Setup local zine repository
        local: bool,
        #[clap(long)]
        /// Location of git repository
        git: Option<String>,
    },
    Sync {},
    Index {
        query: Vec<String>,
    },
    List {},
    Rm {
        #[clap(short, long)]
        force: bool,
        query: String,
    },
    Pdfs {},
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Cli::parse();

    tracing_subscriber::fmt()
        .without_time()
        .with_target(false)
        .with_max_level(args.verbose)
        .init();

    match args.cmd {
        Command::Init { local, git } => {
            let root = if local {
                Some(
                    Utf8PathBuf::from_path_buf(fs::canonicalize(".")?)
                        .map_err(|_| eyre!("Current directory is not valid UTF-8"))?
                        .join(".zime"),
                )
            } else {
                None
            };
            let setup = Setup::new(git, root)?;

            info!(config_dir=%setup.root(), "initalizing...");

            fs::create_dir_all(setup.root())?;

            if let Some(git) = setup.git() {
                debug!(git=%git, "setting up git repository");

                if setup.root().join(".git").exists() {
                    debug!("git repository already exists");
                } else {
                    debug!("creating new git repository");
                    cmd!("git", "init").dir(setup.root()).run()?;
                };

                // ignore the result of this command
                let _ = cmd!("git", "remote", "add", "origin", git)
                    .dir(setup.root())
                    .run();

                match cmd!("git", "pull", "origin", "main")
                    .dir(setup.root())
                    .run()
                {
                    Ok(_) => debug!("pulled from remote"),
                    Err(e) => {
                        debug!(error=%e, "failed to pull from remote, ignoring");
                    }
                }
            } else {
                debug!("no git repository specified");
            }

            debug!(config_file=%setup.config_file(), "checking if config file already exists");

            let config = if setup.config_file().exists() {
                info!(config_file=%setup.config_file(), "using existing config file");
                config::Config::load(&setup.config_file())?
            } else {
                info!(config_file=%setup.config_file(), "creating config file");
                let config = config::Config::default();
                config.write(&setup.config_file())?;
                config
            };

            dbg!(config);

            let _bib = if setup.bib_path().exists() {
                info!(bib=%setup.bib_path(), "using existing bibliography file");
                setup.bib_path()
            } else {
                info!(bib=%setup.bib_path(), "creating bibliography file");
                let bib = setup.bib_path();
                fs::write(&bib, "")?;
                bib
            };

            const GITIGNORE: &str = r#"
pdfs/
.DS_Store
"#;
            let gitignore = setup.root().join(".gitignore");
            if !gitignore.exists() {
                info!(gitignore=%gitignore, "creating gitignore file");
                fs::write(&gitignore, GITIGNORE.trim_start())?;
            }

            setup.sync_git()?;
        }
        Command::Sync {} => {
            let setup = Setup::determine_from_cwd()?;
            setup.sync_git()?;
        }
        Command::Index { query } => {
            let setup = Setup::determine_from_cwd()?;

            let spinner = cliclack::spinner();
            spinner.start("Looking up articles...");
            let res = remotes::dblp::search(&query.join(" "))?;
            spinner.stop("");

            let selection = cliclack::select("Select article")
                .items(
                    &res.result
                        .hits
                        .hit
                        .iter()
                        .map(|hit| {
                            (
                                hit,
                                format!(
                                    "{} ({})",
                                    hit.info.title.bold(),
                                    hit.info
                                        .authors
                                        .author
                                        .iter()
                                        .map(|a| a.text.italic())
                                        .format(", ")
                                ),
                                if let Some(doi) = hit.info.doi.as_ref() {
                                    format!("DOI: {}", doi)
                                } else {
                                    "".to_string()
                                },
                            )
                        })
                        .collect_vec(),
                )
                .interact()?;
            cliclack::outro("Added!")?;

            let spinner = cliclack::spinner();
            spinner.start("Downloading bibliography...");
            let bib_entry = selection.bib()?;
            spinner.stop("");

            let mut bib = setup.bib()?;

            bib.insert(
                biblatex::Bibliography::parse(&bib_entry)
                    .map_err(|err| eyre!("failed to parse bibliography entry: {err}"))?
                    .into_iter()
                    .next()
                    .unwrap(),
            );

            debug!("writing bibliography to file");
            fs::write(setup.bib_path(), bib.to_biblatex_string())?;

            setup.sync_git()?;
        }
        Command::Rm { force, query } => {
            let setup = Setup::determine_from_cwd()?;
            let mut bib = setup.bib()?;

            // find possible entries

            let entries = bib
                .iter()
                .filter(|entry| {
                    entry.doi().map(|doi| doi == query).unwrap_or_default()
                        || entry
                            .title()
                            .map(|title| {
                                title
                                    .to_biblatex_string(false)
                                    .to_lowercase()
                                    .contains(&query.to_lowercase())
                            })
                            .unwrap_or_default()
                })
                .collect_vec();

            if entries.is_empty() {
                return Err(eyre!("No entry found with DOI or title: {}", query));
            }

            let selection = cliclack::select("Select article to remove")
                .items(
                    &entries
                        .iter()
                        .map(|entry| {
                            (
                                entry,
                                format!(
                                    "{} ({})",
                                    entry
                                        .title()
                                        .unwrap_or_default()
                                        .to_biblatex_string(true)
                                        .bold(),
                                    entry
                                        .author()
                                        .unwrap_or_default()
                                        .into_iter()
                                        .map(|a| a.to_string())
                                        .format(", ")
                                ),
                                if let Ok(doi) = entry.doi() {
                                    format!("DOI: {}", doi)
                                } else {
                                    "".to_string()
                                },
                            )
                        })
                        .collect_vec(),
                )
                .interact()?;

            let title = selection
                .title()
                .unwrap_or_default()
                .to_biblatex_string(true);
            if force || cliclack::confirm(format!("Remove {}?", title)).interact()? {
                let key = selection.key.clone();
                let removed = bib.remove(&key);
                if removed.is_none() {
                    return Err(eyre!("Failed to remove entry"));
                }
                fs::write(setup.bib_path(), bib.to_biblatex_string())?;
                setup.sync_git()?;
            }
        }
        Command::List {} => {
            let setup = Setup::determine_from_cwd()?;
            let bib = setup.bib()?;
            for entry in bib {
                let title = entry.title().unwrap_or_default().to_biblatex_string(true);
                let authors = entry
                    .author()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|a| a.to_string())
                    .format(", ");
                let doi = entry.doi().unwrap_or_default();
                println!("{} ({})\n  {}", title.bold(), doi, authors.italic());
            }
        }
        Command::Pdfs {} => {
            let setup = Setup::determine_from_cwd()?;
            let bib = setup.bib()?;
            for entry in bib {
                let doi = match entry.doi() {
                    Ok(doi) => doi,
                    Err(err) => {
                        let title = entry.title().unwrap_or_default().to_biblatex_string(true);
                        warn!(title=%title, %err, "failed to extract DOI");
                        continue;
                    }
                };
                let path = setup.pdf_dir().join(format!("{}.pdf", path_safe_doi(&doi)));

                if path.exists() {
                    debug!(%path, "skipping PDF, already exists");
                    continue;
                }

                let pdf = if is_arxiv(&doi) {
                    match remotes::arxiv::fetch_pdf(&doi) {
                        Ok(pdf) => pdf,
                        Err(err) => {
                            let title = entry.title().unwrap_or_default().to_biblatex_string(true);
                            warn!(title=%title, %doi, %err, "failed to download PDF");
                            continue;
                        }
                    }
                } else {
                    match remotes::scihub::fetch_pdf(&doi) {
                        Ok(pdf) => pdf,
                        Err(err) => {
                            let title = entry.title().unwrap_or_default().to_biblatex_string(true);
                            warn!(title=%title, %doi, %err, "failed to download PDF");
                            continue;
                        }
                    }
                };
                fs::create_dir_all(setup.pdf_dir())?;
                debug!(path=%path, "writing PDF to file");
                fs::write(&path, pdf)?;
                info!(path=%path, "downloaded PDF");
            }
        }
    }

    Ok(())
}

fn path_safe_doi(doi: &str) -> String {
    doi.replace("/", "--")
}

mod config {
    use std::fs;

    use crate::Result;

    use camino::{Utf8Path, Utf8PathBuf};
    use color_eyre::eyre::eyre;
    use duct::cmd;
    use serde::{Deserialize, Serialize};
    use tracing::{debug, info, warn};

    pub struct Setup {
        git: Option<String>,
        config_base: Utf8PathBuf,
    }

    impl Setup {
        pub fn new(git: Option<String>, root: Option<Utf8PathBuf>) -> Result<Self> {
            let config_base = if let Some(root) = root {
                root
            } else {
                global_config_dir()?
            };
            // check if the directory is a git repository
            let git = if cmd!("git", "rev-parse", "--is-inside-work-tree")
                .dir(&config_base)
                .read()
                .is_ok()
            {
                let found = cmd!("git", "remote", "get-url", "origin")
                    .dir(&config_base)
                    .read()?;
                if let Some(given) = git {
                    if given != found {
                        warn!(
                            ?given,
                            ?found,
                            "ignoring git repository specified, using existing repository"
                        );
                    }
                }
                Some(found)
            } else {
                git
            };
            Ok(Self { git, config_base })
        }

        pub fn determine_from_cwd() -> Result<Self> {
            Self::determine_from(
                &Utf8PathBuf::from_path_buf(std::env::current_dir()?)
                    .map_err(|_| eyre!("Current directory is not valid UTF-8"))?,
            )
        }

        pub fn determine_from(path: &Utf8Path) -> Result<Self> {
            // walk up the directory tree until we find a zime.toml file
            let mut current = path;
            loop {
                let config_dir = current.join(".zime");
                if config_dir.exists() {
                    debug!(config_dir=%config_dir, "found config dir");
                    return Self::new(None, Some(config_dir));
                }
                if let Some(parent) = current.parent() {
                    current = parent;
                } else {
                    break;
                }
            }
            // use global config directory
            debug!("using global config directory");
            Self::new(None, None)
        }

        pub fn root(&self) -> Utf8PathBuf {
            self.config_base.clone()
        }

        pub fn config_file(&self) -> Utf8PathBuf {
            self.config_base.join("zime.toml")
        }

        pub fn bib_path(&self) -> Utf8PathBuf {
            self.config_base.join("references.bib")
        }

        pub fn pdf_dir(&self) -> Utf8PathBuf {
            self.config_base.join("pdfs")
        }

        pub fn git(&self) -> Option<&str> {
            self.git.as_deref()
        }

        pub fn sync_git(&self) -> Result<()> {
            if let Some(_git) = self.git() {
                // check for changes
                let status = duct::cmd!("git", "status", "--porcelain")
                    .dir(self.root())
                    .read()?;
                // commit if any
                if !status.is_empty() {
                    info!("committing changes");
                    duct::cmd!("git", "add", ".").dir(self.root()).run()?;
                    duct::cmd!("git", "commit", "-m", "zime: auto commit")
                        .dir(self.root())
                        .run()?;
                }

                // pull from upstream
                duct::cmd!("git", "pull", "origin", "main", "--rebase")
                    .dir(self.root())
                    .run()?;

                // push changes
                if !status.is_empty() {
                    duct::cmd!("git", "push", "origin", "main")
                        .dir(self.root())
                        .run()?;
                }
            }
            Ok(())
        }

        pub fn bib(&self) -> Result<biblatex::Bibliography> {
            if !self.bib_path().exists() {
                fs::write(&self.bib_path(), "")?;
            }
            let src = fs::read_to_string(&self.bib_path())?;
            let bib = biblatex::Bibliography::parse(&src)
                .map_err(|err| eyre!("failed to parse {}: {err}", self.bib_path()))?;
            Ok(bib)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct Config {}

    #[allow(clippy::derivable_impls)]
    impl Default for Config {
        fn default() -> Self {
            Self {}
        }
    }

    impl Config {
        pub fn load(path: &Utf8Path) -> Result<Self> {
            let content = std::fs::read_to_string(path)?;
            toml::from_str(&content).map_err(|e| eyre!(e))
        }
        pub fn write(&self, path: &Utf8Path) -> Result<()> {
            let content = toml::to_string(self)?;
            std::fs::write(path, content)?;
            Ok(())
        }
    }

    fn global_config_dir() -> Result<Utf8PathBuf> {
        let dirs = directories::ProjectDirs::from("com", "zime", "zime")
            .ok_or_else(|| eyre!("Could not determine configuration directory"))?;
        Utf8PathBuf::from_path_buf(dirs.config_dir().to_path_buf())
            .map_err(|_| eyre!("Config path is not valid UTF-8"))
    }
}

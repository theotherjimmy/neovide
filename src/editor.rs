use std::collections::HashMap;

use skulpin::skia_safe::{colors, Color4f};

use neovim_lib::{Neovim, NeovimApi};

#[derive(new, PartialEq, Debug, Clone)]
pub struct Colors {
    pub foreground: Option<Color4f>,
    pub background: Option<Color4f>,
    pub special: Option<Color4f>
}

#[derive(new, Debug, Clone, PartialEq)]
pub struct Style {
    pub colors: Colors,
    #[new(default)]
    pub reverse: bool,
    #[new(default)]
    pub italic: bool,
    #[new(default)]
    pub bold: bool,
    #[new(default)]
    pub strikethrough: bool,
    #[new(default)]
    pub underline: bool,
    #[new(default)]
    pub undercurl: bool,
    #[new(default)]
    pub blend: u8
}

#[derive(new)]
pub struct GridLineCell {
    pub grid: usize,
    pub text: String,
    pub row: usize,
    pub col_start: usize,
    pub style_id: Option<u64>
}

pub type GridCell = Option<(char, Style)>;

#[derive(new, Debug, Clone)]
pub struct DrawCommand {
    pub text: String,
    pub row: usize,
    pub col_start: usize,
    pub style: Style
}

#[derive(Clone)]
pub enum CursorType {
    Block,
    Horizontal,
    Vertical
}

pub struct Editor {
    pub nvim: Neovim,
    pub grid: Vec<Vec<GridCell>>,
    pub cursor_pos: (usize, usize),
    pub cursor_type: CursorType,
    pub size: (usize, usize),
    pub default_colors: Colors,
    pub defined_styles: HashMap<u64, Style>,
    pub previous_style: Option<Style>
}

impl Editor {
    pub fn new(nvim: Neovim, width: usize, height: usize) -> Editor {
        let mut editor = Editor {
            nvim,
            grid: Vec::new(),
            cursor_pos: (0, 0),
            cursor_type: CursorType::Block,
            size: (width, height),
            default_colors: Colors::new(Some(colors::WHITE), Some(colors::BLACK), Some(colors::GREY)),
            defined_styles: HashMap::new(),
            previous_style: None
        };
        editor.clear();
        editor
    }

    pub fn build_draw_commands(&self) -> Vec<DrawCommand> {
        self.grid.iter().enumerate().map(|(row_index, row)| {
            let mut draw_commands = Vec::new();
            let mut command = None;

            fn add_command(commands_list: &mut Vec<DrawCommand>, command: Option<DrawCommand>) {
                if let Some(command) = command {
                    commands_list.push(command);
                }
            }

            fn command_matches(command: &Option<DrawCommand>, style: &Style) -> bool {
                match command {
                    Some(command) => &command.style == style,
                    None => true
                }
            }

            fn add_character(command: &mut Option<DrawCommand>, character: &char, row_index: usize, col_index: usize, style: Style) {
                match command {
                    Some(command) => command.text.push(character.clone()),
                    None => {
                        command.replace(DrawCommand::new(character.to_string(), row_index, col_index, style));
                    }
                }
            }

            for (col_index, cell) in row.iter().enumerate() {
                if let Some((character, new_style)) = cell {
                    if !command_matches(&command, &new_style) {
                        add_command(&mut draw_commands, command);
                        command = None;
                    }
                    add_character(&mut command, &character, row_index as usize, col_index as usize, new_style.clone());
                } else {
                    add_command(&mut draw_commands, command);
                    command = None;
                }
            }
            add_command(&mut draw_commands, command);

            draw_commands
        }).flatten().collect()
    }

    pub fn draw(&mut self, command: GridLineCell) {
        let row_index = command.row as usize;
        let col_start = command.col_start as usize;

        let style = match (command.style_id, self.previous_style.clone()) {
            (Some(0), _) => Style::new(self.default_colors.clone()),
            (Some(style_id), _) => self.defined_styles.get(&style_id).expect("GridLineCell must use defined color").clone(),
            (None, Some(previous_style)) => previous_style,
            (None, None) => Style::new(self.default_colors.clone())
        };

        if row_index < self.grid.len() {
            let row = self.grid.get_mut(row_index).expect("Grid must have size greater than row_index");
            for (i, character) in command.text.chars().enumerate() {
                let pointer_index = i + col_start;
                if pointer_index < row.len() {
                    row[pointer_index] = Some((character, style.clone()));
                }
            }
        } else {
            println!("Draw command out of bounds");
        }

        self.previous_style = Some(style);
    }

    pub fn scroll_region(&mut self, top: isize, bot: isize, left: isize, right: isize, rows: isize, cols: isize) {
        let (top, bot) =  if rows > 0 {
            (top + rows, bot)
        } else if rows < 0 {
            (top, bot + rows)
        } else {
            (top, bot)
        };

        let (left, right) = if cols > 0 {
            (left + cols, right)
        } else if rows < 0 {
            (left, right + cols)
        } else {
            (left, right)
        };

        let width = right - left;
        let height = bot - top;

        let mut region = Vec::new();
        for y in top..bot {
            let row = &self.grid[y as usize];
            let mut copied_section = Vec::new();
            for x in left..right {
                copied_section.push(row[x as usize].clone());
            }
            region.push(copied_section);
        }

        let new_top = top as isize - rows;
        let new_left = left as isize - cols;

        dbg!(top, bot, left, right, rows, cols, new_top, new_left);

        for (y, row_section) in region.into_iter().enumerate() {
            for (x, cell) in row_section.into_iter().enumerate() {
                let y = new_top + y as isize;
                if y >= 0 && y < self.grid.len() as isize {
                    let mut row = &mut self.grid[y as usize];
                    let x = new_left + x as isize;
                    if x >= 0 && x < row.len() as isize {
                        row[x as usize] = cell;
                    }
                }
            }
        }
    }


    pub fn clear(&mut self) {
        let (width, height) = self.size;
        self.grid = vec![vec![None; width as usize]; height as usize];
    }

    pub fn resize(&mut self, new_width: usize, new_height: usize) {
        self.nvim.ui_try_resize(new_width as i64, new_height as i64).expect("Resize failed");
        self.size = (new_width, new_height);
    }

    pub fn define_style(&mut self, id: u64, style: Style) {
        self.defined_styles.insert(id, style);
    }

    pub fn set_default_colors(&mut self, foreground: Color4f, background: Color4f, special: Color4f) {
        self.default_colors = Colors::new(Some(foreground), Some(background), Some(special));
    }

    pub fn jump_cursor_to(&mut self, row: usize, col: usize) {
        self.cursor_pos = (row, col);
    }
}
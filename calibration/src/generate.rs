use opencv::{
    objdetect::{self, CharucoBoard},
    prelude::*,
    imgcodecs,
    core,
};

pub fn generate_board(width: i32, height: i32) -> opencv::Result<CharucoBoard> {
    CharucoBoard::new(
        core::Size { width, height },
        0.04,
        0.02,
        &objdetect::get_predefined_dictionary(objdetect::PredefinedDictionaryType::DICT_4X4_50)?,
        &core::no_array(),
    )
}

pub fn export_board(
    board: &CharucoBoard,
    margin: i32,
    res: i32,
    name: &String,
) -> opencv::Result<()> {
    let mut img = Mat::default();
    let size = board.get_chessboard_size()?;
    board.generate_image(
        core::Size {
            width: size.width * res,
            height: size.height * res,
        },
        &mut img,
        margin,
        1,
    )?;
    imgcodecs::imwrite(name.as_str(), &img, &core::Vector::<i32>::default())?;
    println!("board successfully exported to `{name}`");
    Ok(())
}

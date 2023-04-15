use opencv::{
    core::Size,
    imgcodecs,
    objdetect::{self, CharucoBoard},
    prelude::*,
};

pub fn generate_board(width: i32, height: i32) -> opencv::Result<CharucoBoard> {
    Ok(CharucoBoard::new(
        Size { width, height },
        0.04,
        0.02,
        &objdetect::get_predefined_dictionary(objdetect::PredefinedDictionaryType::DICT_4X4_50)?,
        &opencv::core::no_array(),
    )?)
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
        Size {
            width: size.width * res,
            height: size.height * res,
        },
        &mut img,
        margin,
        1,
    )?;
    imgcodecs::imwrite(name.as_str(), &img, &opencv::core::Vector::<i32>::default())?;
    Ok(())
}

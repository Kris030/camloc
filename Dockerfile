FROM agners/archlinuxarm

# update
RUN pacman --noconfirm -Syu

# install rust
RUN pacman --noconfirm -S rust

# install opencv deps
RUN pacman --noconfirm -S clang base-devel qt5-base opencv vtk hdf5 glew fmt openmpi

WORKDIR /app

CMD ["cargo", "build", "--release"]

#!/usr/bin/env python3

import hashlib
import shutil
from pathlib import Path

from faker import Faker

SLIDE_DEFAULT_NAME = "Slides"
volumes = ["Laptop", "Server", "Pendrive"]


def del_folder(p):
    try:
        shutil.rmtree(p, ignore_errors=True)
        p.rmdir()
    except Exception:
        pass


if __name__ == "__main__":
    script_path = Path(__file__).resolve().parent

    Faker.seed(0)
    fake = Faker()

    original_folder = script_path.joinpath("original")
    processed_folder = script_path.joinpath("processed")
    expected_folder = script_path.joinpath("expected")

    del_folder(original_folder)
    del_folder(processed_folder)
    del_folder(expected_folder)
    original_folder.mkdir(parents=True, exist_ok=True)

    for volume in volumes:
        for slide in volumes:
            num_of_folders = fake.random_int(min=1, max=5)

            for _ in range(num_of_folders):
                folder_kind = fake.random_element(
                    elements=("audio", "video", "image", "office", "text")
                )

                folder_path = (
                    original_folder.joinpath(volume)
                    .joinpath(SLIDE_DEFAULT_NAME)
                    .joinpath(slide)
                    .joinpath(folder_kind)
                )
                folder_path.mkdir(parents=True, exist_ok=True)
                print(f"{folder_path} created")

                (
                    expected_folder.joinpath(volume)
                    .joinpath(SLIDE_DEFAULT_NAME)
                    .joinpath(slide)
                    .joinpath(folder_kind)
                ).mkdir(parents=True, exist_ok=True)

                folder_path_ = (
                    expected_folder.joinpath(slide)
                    .joinpath(SLIDE_DEFAULT_NAME)
                    .joinpath(slide)
                    .joinpath(folder_kind)
                )
                folder_path_.mkdir(parents=True, exist_ok=True)

                num_of_files = fake.random_int(min=1, max=10)

                for _ in range(num_of_files):
                    file_path = folder_path.joinpath(
                        fake.file_name(category=folder_kind)
                    )
                    num_of_bytes = fake.random_int(min=1, max=10) * 1024

                    file_path.write_text(fake.text(max_nb_chars=num_of_bytes))

                    sha256_hash = hashlib.sha256()
                    with open(file_path, "rb") as f, open(
                        f"{file_path}.sha256", "w"
                    ) as cf:
                        for byte_block in iter(lambda: f.read(4096), b""):
                            sha256_hash.update(byte_block)
                        checksum = sha256_hash.hexdigest()
                        cf.write(checksum)

                    shutil.copy(file_path, folder_path_)
                    shutil.copy(f"{file_path}.sha256", folder_path_)

    shutil.copytree(original_folder, processed_folder)
    shutil.copy(script_path.joinpath("test.conf"), processed_folder)

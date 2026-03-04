from main_types import Student


async def getStudentDetails(id: str) -> Student:
    return {
        "user_name": "Babyface",
        "age": 22,
        "id": id,
        "grades": ["A", "A+"]
    }
